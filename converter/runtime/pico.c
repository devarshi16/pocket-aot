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
// Frames cross cores packed at 4 bits per pixel, as in PICO-8 screen memory.
// Core 0 copies a finished frame straight into display_frame while core 1 is
// not drawing it; a frame offered while core 1 is busy is offered again later.
static uint8_t display_frame[128*64];
static uint8_t display_palette[16];
// Stream a scanline at a time; a full 115 KB RGB565 buffer is unnecessary.
static uint8_t lcd_bytes[240*2];
static uint16_t palette565[32];
static uint8_t scale_lut[240];
static bool frame_ready, rendering, missed;
static volatile uint32_t video_frames, actual_spi, display_ms;
static volatile unsigned speed_index;
static bool paused;
static uint32_t logic_frames;
static uint16_t physical_buttons;
static bool failed;
static uint8_t injected;
static uint32_t inject_until;

/* Leave enough space for the next measured native frame and error recovery. */
extern uint8_t __StackBottom, __StackTop;
void p8_stack_check(void){
    uintptr_t sp;__asm volatile("mov %0, sp":"=r"(sp));
    // Coroutines run on small runtime-owned stacks with a smaller guard.
    const void *low=p8_current_stack_low();
    if(low){if(sp<(uintptr_t)low+768)p8_fail("coroutine stack budget exceeded");return;}
    if(sp<(uintptr_t)&__StackBottom+4096)p8_fail("native stack budget exceeded");
}

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
    for (int i=0;i<32;i++) {
        const uint8_t *c=pico_palette_extended[i];
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
        if (frame_ready) { frame_ready=false; rendering=true; ready=true; }
        critical_section_exit(&video_lock);
        if (!ready) { sleep_us(200); continue; }
        if (speed_index!=last_speed) {
            last_speed=speed_index; actual_spi=spi_set_baudrate(spi1,spi_speeds[last_speed]);
        }
        uint32_t start=time_us_32();
        // The screen palette (0x5f10..) maps frame indices to 32 display colours.
        uint16_t lut[16];
        for (int i=0;i<16;i++) { int c=display_palette[i]; lut[i]=palette565[(c&128)?16+(c&15):(c&15)]; }
        panel_begin();
        for (int y=0;y<240;y++) {
            const uint8_t *row=display_frame+scale_lut[y]*64;
            uint8_t *out=lcd_bytes;
            for (int x=0;x<240;x++) {
                unsigned sx=scale_lut[x];
                uint16_t c=lut[(row[sx>>1]>>((sx&1)*4))&15];
                *out++=c>>8; *out++=c;
            }
            spi_write_blocking(spi1,lcd_bytes,sizeof lcd_bytes);
        }
        gpio_put(LCD_CS,1);
        critical_section_enter_blocking(&video_lock); rendering=false; critical_section_exit(&video_lock);
        display_ms=(time_us_32()-start)/1000;
        video_frames++;
    }
}

static bool offer_frame(void) {
    critical_section_enter_blocking(&video_lock);
    bool taken=!rendering;
    if (taken) { memcpy(display_frame,game_frame,sizeof display_frame); memcpy(display_palette,p8_screen_palette,sizeof display_palette); frame_ready=true; }
    critical_section_exit(&video_lock);
    return taken;
}
/* game_frame stays unchanged until the next frame starts, so a missed frame is offered again while waiting. */
static void publish(void) { missed=!offer_frame(); }
/* For frames drawn over game_frame and then undone, such as the pause menu. */
static void publish_now(void) { while (!offer_frame()) tight_loop_contents(); missed=false; }

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
    memset(game_frame,0x11,sizeof game_frame);
    V zero=p8_int(0);p8_api("camera",0,NULL);p8_api("clip",0,NULL);
    p8_text("CONVERSION RUNTIME ERROR",10,10,8);
    for(unsigned i=0;i<strlen(p8_error);i+=27){char line[28];snprintf(line,sizeof line,"%.27s",p8_error+i);p8_text(line,8,27+(i/27)*8,7);}
    p8_text("X+Y: USB RECOVERY",20,106,6);
}
static void restart(void);
static void status(void);
static int menu_cursor;
/* Pause overlay: cartridge menuitem() entries followed by CONTINUE. */
static void draw_pause_menu(void) {
    /* Only the menu box is drawn over, so only it is saved: six rows at most. */
    /* 80 pixels from x=24 are 40 whole bytes of the packed screen. */
    static uint8_t saved[(6*8+8)*40];
    int items=p8_menu_count(),rows=items+1,h=rows*8+8,y0=64-h/2;
    for (int y=0;y<h;y++) memcpy(saved+y*40,game_frame+(y0+y)*64+12,40);
    for (int y=y0;y<y0+h;y++) memset(game_frame+y*64+12,0,40);
    int row=0;
    for (int i=1;i<=5;i++) {
        const char *label=p8_menu_label(i); if(!label) continue;
        char text[17];snprintf(text,sizeof text,"%s",label);p8_overlay_text(text,36,y0+5+row*8,menu_cursor==row?7:6); row++;
    }
    p8_overlay_text("continue",36,y0+5+row*8,menu_cursor==row?7:6);
    p8_overlay_text(">",28,y0+5+menu_cursor*8,7);
    publish_now();
    for (int y=0;y<h;y++) memcpy(game_frame+(y0+y)*64+12,saved+y*40,40);
}
static int menu_entry(int row) { int seen=0; for (int i=1;i<=5;i++) if (p8_menu_label(i)) { if (seen==row) return i; seen++; } return 0; }
/* Returns true while the game should stay paused. */
static bool pause_input(uint16_t latched) {
    int rows=p8_menu_count()+1;
    if (latched&(1u<<2)) menu_cursor=(menu_cursor+rows-1)%rows;
    if (latched&(1u<<3)) menu_cursor=(menu_cursor+1)%rows;
    if (latched&((1u<<4)|(1u<<5))) {
        int entry=menu_entry(menu_cursor);
        if (!entry) return false;
        p8_menu_select(entry,latched&3);
        return false;
    }
    if (latched&((1u<<6)|(1u<<8))) return false;
    return true;
}
static uint32_t next_frame;
static uint16_t last_buttons,latched_buttons;
static uint32_t recovery_start;
static void service_usb(uint32_t now) {
    int ch=getchar_timeout_us(0);
    if(ch=='s')status();else if(ch=='b')reset_usb_boot(0,0);else if(ch=='p')paused=!paused;
    else if(ch=='i'){int value=getchar_timeout_us(10000);if(value>=0){injected=value&63;inject_until=now+250000;}}
    else if(ch=='f'){printf("FRAME 16384\n");for(int i=0;i<128*128;i++)putchar(p8_frame_get(i&127,i>>7));fflush(stdout);}
}
static void poll_buttons(uint32_t now) {
    physical_buttons=read_buttons(now);latched_buttons|=physical_buttons&~last_buttons;last_buttons=physical_buttons;
    if((physical_buttons&0xc0)==0xc0){if(!recovery_start)recovery_start=now;if(now-recovery_start>2000000)reset_usb_boot(0,0);}else recovery_start=0;
}
/* Wait for the next frame slot; returns the buttons held for that frame. */
static uint8_t wait_frame(void) {
    for (;;) {
        uint32_t now=time_us_32();poll_buttons(now);service_usb(now);
        if(missed)publish();
        if((int32_t)(now-next_frame)>=0){
            unsigned interval=1000000/p8_target_hz;next_frame+=interval;if((int32_t)(now-next_frame)>100000)next_frame=now+interval;
            uint16_t latched=latched_buttons;latched_buttons=0;
            if(latched&(1u<<7))speed_index=(speed_index+1)%3;
            if(p8_paused_by_cart){p8_paused_by_cart=false;paused=true;menu_cursor=0;latched=0;}
            if(!paused&&(latched&((1u<<6)|(1u<<8)))){paused=true;menu_cursor=0;latched=0;}
            if(paused){paused=pause_input(latched);if(paused)draw_pause_menu();else publish();logic_frames++;continue;}
            uint8_t held=(physical_buttons|latched)&63;if((int32_t)(inject_until-now)>0)held|=injected;
            return held;
        }
        sleep_us(300);
    }
}
/* flip(): publish the frame drawn so far and continue in the same call stack. */
void p8_present(void) {
    p8_flip_collect();publish();logic_frames++;
    p8_flip_frame(wait_frame());
}
static void restart(void) {
    failed=false;paused=false;
    int jump=setjmp(p8_guard);
    if(jump==P8_RESTART){restart();return;}
    if(jump){failed=true;printf("AOT_ERROR %s\n",p8_error);error_screen();return;}
    p8_reset();p8_program_init();p8_collect();
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
    p8_stack_top=&__StackTop;
    panel_init();critical_section_init(&video_lock);next_frame=time_us_32();restart();multicore_launch_core1(video_core);publish();
    for(;;){
        if(failed){uint32_t now=time_us_32();poll_buttons(now);if(missed)publish();int ch=getchar_timeout_us(0);if(ch=='r')restart();else if(ch=='s')status();else if(ch=='b')reset_usb_boot(0,0);sleep_us(1000);continue;}
        uint8_t held=wait_frame();
        int jump=setjmp(p8_guard);
        if(jump==P8_RESTART){restart();}
        else if(jump){failed=true;printf("AOT_ERROR %s\n",p8_error);error_screen();publish();}
        else{p8_begin_frame(held);p8_program_update();p8_program_draw();p8_end_frame();publish();logic_frames++;}
    }
}
