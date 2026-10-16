#include "p8.h"
#include <stdio.h>
#include <stdlib.h>

static void dump(const char *name){char text[128];V v=p8_debug_global(name);printf("%s=%s ",name,p8_describe(v,text,sizeof text));}
int main(int argc,char **argv){
 int frames=argc>1?atoi(argv[1]):1200;
 if(setjmp(p8_guard)){fprintf(stderr,"CARTRIDGE_ERROR: %s frame=%u\n",p8_error,p8_frames);
#ifdef P8_API_PROFILE
 p8_profile_write("api-profile.json");
#endif
 return 1;}
 p8_reset();p8_program_init();p8_program_draw();p8_collect();
 FILE *title=fopen("title.idx","wb");if(title){fwrite(game_frame,1,sizeof game_frame,title);fclose(title);}
 for(int i=0;i<frames;i++){
  uint8_t input=0;
  // Start with B; launch with A; move the paddle under the ball on the host.
  V mode=p8_debug_global("mode");char m[128];p8_describe(mode,m,sizeof m);
  if(!strcmp(m,"start")||!strcmp(m,"gameover"))input=32;
  else{
   V ball=p8_debug_global("ball_x"),pad=p8_debug_global("pad_x"),width=p8_debug_global("pad_w");
   if(ball.tag==P8_NUM&&pad.tag==P8_NUM&&width.tag==P8_NUM){int64_t delta=(int64_t)ball.n-pad.n-width.n/2;if(delta< -65536)input|=1;else if(delta>65536)input|=2;}else input|=1u<<((i/120)%4);
   if((i%120)==5)input|=16;
  }
  p8_begin_frame(input);p8_program_update();p8_program_draw();p8_end_frame();
  if(i==300){FILE *f=fopen("gameplay.idx","wb");if(f){fwrite(game_frame,1,sizeof game_frame,f);fclose(f);}}
 }
 printf("AOT_HOST_OK frames=%u entries=%u strings=%u operations=%u ",p8_frames,p8_heap_entries,p8_heap_strings,p8_operations);
 for(const char **n=(const char*[]){"mode","points","lives","pad_x","ball_x","state","score",NULL};*n;n++)dump(*n);printf("\n");
#ifdef P8_API_PROFILE
 p8_profile_write("api-profile.json");
#endif
 return 0;
}
