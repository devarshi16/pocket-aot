#include "p8.h"
#include <stdio.h>
#include <stdlib.h>

/* The collector scans the native stack for roots, so locals must stay on it:
 * AddressSanitizer's heap-allocated "fake stack" frames would hide them. */
const char *__asan_default_options(void){return "detect_stack_use_after_return=0";}
static int frames=1200,frame_no;
static bool title_written;
static void dump(const char *name){char text[128];V v=p8_debug_global(name);printf("%s=%s ",name,p8_describe(v,text,sizeof text));}
/* One byte per pixel, as the preview and PNG tools read it. */
static void write_frame(const char *path){FILE *f=fopen(path,"wb");if(f){for(int i=0;i<128*128;i++)fputc(p8_frame_get(i&127,i>>7),f);fclose(f);}}
static uint8_t scripted_input(int i){
 uint8_t input=0;
 // Start with B; launch with A; move the paddle under the ball on the host.
 V mode=p8_debug_global("mode");char m[128];p8_describe(mode,m,sizeof m);
 if(!strcmp(m,"start")||!strcmp(m,"gameover"))input=32;
 else{
  V ball=p8_debug_global("ball_x"),pad=p8_debug_global("pad_x"),width=p8_debug_global("pad_w");
  if(ball.tag==P8_NUM&&pad.tag==P8_NUM&&width.tag==P8_NUM){int64_t delta=(int64_t)ball.n-pad.n-width.n/2;if(delta< -65536)input|=1;else if(delta>65536)input|=2;}else input|=1u<<((i/120)%4);
  if((i%120)==5)input|=16;
 }
 return input;
}
static void finish(void){
 printf("AOT_HOST_OK frames=%u entries=%u strings=%u operations=%u ",p8_frames,p8_heap_entries,p8_heap_strings,p8_operations);
 for(const char **n=(const char*[]){"mode","points","lives","pad_x","ball_x","state","score",NULL};*n;n++)dump(*n);printf("\n");
#ifdef P8_API_PROFILE
 p8_profile_write("api-profile.json");
#endif
 fflush(stdout);exit(0);
}
/* flip() from generated code: present this frame and continue with new input. */
void p8_present(void){
 if(!title_written){write_frame("title.idx");title_written=true;}
 p8_flip_collect();frame_no++;
 if(frame_no==300)write_frame("gameplay.idx");
 if(frame_no>=frames)finish();
 p8_flip_frame(scripted_input(frame_no));
}
int main(int argc,char **argv){
 frames=argc>1?atoi(argv[1]):1200;
 p8_stack_top=__builtin_frame_address(0);
 int jump=setjmp(p8_guard);
 if(jump==1){fprintf(stderr,"CARTRIDGE_ERROR: %s frame=%u\n",p8_error,p8_frames);
#ifdef P8_API_PROFILE
 p8_profile_write("api-profile.json");
#endif
 return 1;}
 // run() restarts the cartridge without resetting the scripted frame count.
 p8_reset();p8_program_init();p8_collect();
 for(;frame_no<frames;frame_no++){
  p8_begin_frame(scripted_input(frame_no));p8_program_update();p8_program_draw();p8_end_frame();
  if(!title_written){write_frame("title.idx");title_written=true;}
  if(frame_no==300)write_frame("gameplay.idx");
 }
 finish();
 return 0;
}
