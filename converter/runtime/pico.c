/* Hardware frontend for generated Pocket AOT cartridges. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "pico/stdlib.h"
#include "pico/multicore.h"
#include "pico/bootrom.h"
#include "pico/sync.h"
#include "hardware/spi.h"
#include "hardware/pwm.h"
#include "p8.h"
#include "pico/stdio_usb.h"

enum { LCD_CS=9, LCD_DC=8, LCD_RST=12, LCD_BL=13 };
// Hardware order: left, right, up, down, A, B, X, Y, joystick click.
static const uint8_t buttons[] = {16,20,2,18,15,17,19,21,3};
static const uint32_t spi_speeds[] = {40000000,20000000,10000000};
static critical_section_t video_lock;
static uint8_t pending_frame[128*128], display_frame[128*128];
// Stream a scanline at a time; a full 115 KB RGB565 buffer is unnecessary.
static uint8_t lcd_bytes[240*2];
static uint16_t palette565[16];
static uint8_t scale_lut[240];
static bool pending;
static volatile uint32_t video_frames, actual_spi, display_ms;
static volatile unsigned speed_index;
static bool paused;
static uint32_t logic_frames;
static uint16_t physical_buttons;
static bool failed;
static uint8_t injected;
static uint32_t inject_until;

/* Leave enough space for the next measured native frame and error recovery. */
extern uint8_t __StackBottom;
void p8_stack_check(void){uintptr_t sp;__asm volatile("mov %0, sp":"=r"(sp));if(sp<(uintptr_t)&__StackBottom+4096)p8_fail("native stack budget exceeded");}

static void command(uint8_t value) {
    gpio_put(LCD_CS,1); gpio_put(LCD_DC,0); gpio_put(LCD_CS,0);
    spi_write_blocking(spi1,&value,1); gpio_put(LCD_CS,1);
}

static void data(uint8_t value) {
    gpio_put(LCD_CS,1); gpio_put(LCD_DC,1); gpio_put(LCD_CS,0);
    spi_write_blocking(spi1,&value,1); gpio_put(LCD_CS,1);
}

static void panel_init(void) {
    const uint8_t pins[] = {LCD_CS,LCD_DC,LCD_RST};
    for (unsigned i=0;i<sizeof pins;i++) {
        gpio_init(pins[i]); gpio_set_dir(pins[i],GPIO_OUT); gpio_put(pins[i],1);
    }
    spi_init(spi1,spi_speeds[0]);
    gpio_set_function(10,GPIO_FUNC_SPI); gpio_set_function(11,GPIO_FUNC_SPI);
    spi_set_format(spi1,8,SPI_CPOL_0,SPI_CPHA_0,SPI_MSB_FIRST);
    gpio_put(LCD_RST,0); sleep_ms(20); gpio_put(LCD_RST,1); sleep_ms(120);
    // The same controller settings and CS/DC sequencing as the working vendor driver.
    static const uint8_t init[] = {
        0x36,1,0x70, 0x3a,1,0x05, 0xb2,5,0x0c,0x0c,0,0x33,0x33,
        0xb7,1,0x35, 0xbb,1,0x19, 0xc0,1,0x2c, 0xc2,1,0x01,
        0xc3,1,0x12, 0xc4,1,0x20, 0xc6,1,0x0f, 0xd0,2,0xa4,0xa1,
        0xe0,14,0xd0,0x04,0x0d,0x11,0x13,0x2b,0x3f,0x54,0x4c,0x18,0x0d,0x0b,0x1f,0x23,
        0xe1,14,0xd0,0x04,0x0c,0x11,0x13,0x2c,0x3f,0x44,0x51,0x2f,0x1f,0x1f,0x20,0x23
    };
    for (unsigned i=0;i<sizeof init;) {
        command(init[i++]); unsigned n=init[i++]; while(n--) data(init[i++]);
    }
    command(0x21); command(0x11); sleep_ms(120); command(0x29); sleep_ms(20);
    gpio_set_function(LCD_BL,GPIO_FUNC_PWM);
    unsigned slice=pwm_gpio_to_slice_num(LCD_BL);
    pwm_set_wrap(slice,65535); pwm_set_gpio_level(LCD_BL,45000); pwm_set_enabled(slice,true);
    for (int i=0;i<16;i++) {
        const uint8_t *c=pico_palette[i];
        palette565[i]=((c[0]&248)<<8)|((c[1]&252)<<3)|(c[2]>>3);
    }
    for (int x=0;x<240;x++) scale_lut[x]=x*128/240;
}

static void panel_begin(void) {
    command(0x2a); data(0); data(0); data(0); data(239);
    command(0x2b); data(0); data(0); data(0); data(239);
    command(0x2c);
    gpio_put(LCD_CS,1); gpio_put(LCD_DC,1); gpio_put(LCD_CS,0);
}

static void video_core(void) {
    unsigned last_speed=99;
    for (;;) {
        bool ready=false;
        critical_section_enter_blocking(&video_lock);
        if (pending) { memcpy(display_frame,pending_frame,sizeof display_frame); pending=false; ready=true; }
        critical_section_exit(&video_lock);
        if (!ready) { sleep_us(200); continue; }
        if (speed_index!=last_speed) {
            last_speed=speed_index; actual_spi=spi_set_baudrate(spi1,spi_speeds[last_speed]);
        }
        uint32_t start=time_us_32();
        panel_begin();
        for (int y=0;y<240;y++) {
            const uint8_t *row=display_frame+scale_lut[y]*128;
            uint8_t *out=lcd_bytes;
            for (int x=0;x<240;x++) {
                uint16_t c=palette565[row[scale_lut[x]]&15];
                *out++=c>>8; *out++=c;
            }
            spi_write_blocking(spi1,lcd_bytes,sizeof lcd_bytes);
        }
        gpio_put(LCD_CS,1);
        display_ms=(time_us_32()-start)/1000;
        video_frames++;
    }
}

static void publish(void) {
    critical_section_enter_blocking(&video_lock);
    memcpy(pending_frame,game_frame,sizeof pending_frame); pending=true;
    critical_section_exit(&video_lock);
}

static uint16_t read_buttons(uint32_t now) {
    static uint16_t raw,stable;
    static uint32_t changed[9];
    for (unsigned i=0;i<sizeof buttons;i++) {
        uint16_t bit=1u<<i;
        bool pressed=!gpio_get(buttons[i]);
        if (pressed!=!!(raw&bit)) { raw^=bit; changed[i]=now; }
        if (pressed!=!!(stable&bit) && now-changed[i]>=8000) stable^=bit;
    }
    return stable;
}


static void error_screen(void) {
    memset(game_frame,1,sizeof game_frame);
    V zero=p8_int(0);p8_api("camera",0,NULL);p8_api("clip",0,NULL);
    p8_text("CONVERSION RUNTIME ERROR",10,10,8);
    for(unsigned i=0;i<strlen(p8_error);i+=27){char line[28];snprintf(line,sizeof line,"%.27s",p8_error+i);p8_text(line,8,27+(i/27)*8,7);}
    p8_text("X+Y: USB RECOVERY",20,106,6);
}
static void restart(void) {
    failed=false;paused=false;
    if(setjmp(p8_guard)){failed=true;printf("AOT_ERROR %s\n",p8_error);error_screen();return;}
    p8_reset();p8_program_init();p8_program_draw();p8_collect();
    printf("AOT_READY hz=%u title=%s\n",p8_target_hz,p8_title);
}
static void status(void) {
    char mode[128],score[128],pad[128];
    p8_describe(p8_debug_global("mode"),mode,sizeof mode);
    p8_describe(p8_debug_global("points"),score,sizeof score);
    p8_describe(p8_debug_global("pad_x"),pad,sizeof pad);
    printf("AOT_STATUS logic=%lu video=%lu transfer_ms=%lu spi=%lu error=%u paused=%u buttons=%u heap=%lu strings=%lu mode=%s points=%s pad_x=%s title=%s\n",
     (unsigned long)p8_frames,(unsigned long)video_frames,(unsigned long)display_ms,(unsigned long)actual_spi,failed,paused,physical_buttons,
     (unsigned long)p8_heap_entries,(unsigned long)p8_heap_strings,mode,score,pad,p8_title);
}
int main(void) {
    stdio_init_all();stdio_set_translate_crlf(&stdio_usb,false);
    for(unsigned i=0;i<sizeof buttons;i++){gpio_init(buttons[i]);gpio_set_dir(buttons[i],GPIO_IN);gpio_pull_up(buttons[i]);}
    panel_init();critical_section_init(&video_lock);restart();multicore_launch_core1(video_core);publish();
    uint32_t next=time_us_32(),recovery_start=0;uint16_t last=0,latched=0;
    for(;;){
        uint32_t now=time_us_32();physical_buttons=read_buttons(now);latched|=physical_buttons&~last;last=physical_buttons;
        if((physical_buttons&0xc0)==0xc0){if(!recovery_start)recovery_start=now;if(now-recovery_start>2000000)reset_usb_boot(0,0);}else recovery_start=0;
        int ch=getchar_timeout_us(0);
        if(ch=='s')status();else if(ch=='b')reset_usb_boot(0,0);else if(ch=='r')restart();else if(ch=='p')paused=!paused;
        else if(ch=='i'){int value=getchar_timeout_us(10000);if(value>=0){injected=value&63;inject_until=now+250000;}}
        else if(ch=='f'){printf("FRAME 16384\n");fwrite(game_frame,1,sizeof game_frame,stdout);fflush(stdout);}
        if((int32_t)(now-next)>=0){
            unsigned interval=1000000/p8_target_hz;next+=interval;if((int32_t)(now-next)>100000)next=now+interval;
            if(latched&((1u<<6)|(1u<<8)))paused=!paused;
            if(latched&(1u<<7))speed_index=(speed_index+1)%3;
            if(!paused&&!failed){
                if(setjmp(p8_guard)){failed=true;printf("AOT_ERROR %s\n",p8_error);error_screen();}
                else{uint8_t held=(physical_buttons|latched)&63;if((int32_t)(inject_until-now)>0)held|=injected;p8_begin_frame(held);p8_program_update();p8_program_draw();p8_end_frame();}
            }
            publish();logic_frames++;latched=0;
        }
        sleep_us(300);
    }
}
