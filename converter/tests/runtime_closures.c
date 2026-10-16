#include "p8.h"
#include <stdio.h>
#include <stdlib.h>
const uint8_t p8_cart_gfx[8192]={0},p8_cart_map[4096]={0},p8_cart_flags[256]={0};
const char *p8_title="closure runtime checks";const unsigned p8_target_hz=30;
static V root;
#define CHECK(x) do{if(!(x)){fprintf(stderr,"failed line %d\n",__LINE__);exit(2);}}while(0)
V p8_dispatch(int fn,V self,int argc,const V *argv){(void)argc;(void)argv;CHECK(fn==0);V c=p8_capture(self,0);V v=p8_binary("+",p8_cell_get(c),p8_int(1));p8_cell_set(c,v);V results[]={v,p8_int(42)};return p8_return(2,results);}
void p8_program_roots(void){p8_mark(root);}
void p8_program_clear(void){root=p8_nil();}
void p8_program_init(void){}void p8_program_update(void){}void p8_program_draw(void){}
V p8_debug_global(const char *name){(void)name;return p8_nil();}
int main(void){if(setjmp(p8_guard)){fprintf(stderr,"%s\n",p8_error);return 1;}p8_reset();V cell=p8_cell(p8_int(0));root=p8_closure(0,1,&cell);
for(int f=0;f<2000;f++){p8_begin_frame(0);CHECK(p8_call(root,0,NULL).n==(f+1)*65536);CHECK(p8_result_count()==2);CHECK(p8_result(1).n==42*65536);CHECK(p8_result(2).tag==P8_NIL);V garbage=p8_table();p8_set(garbage,p8_str("closure"),p8_closure(0,1,&cell));p8_end_frame();}
p8_begin_frame(0);V t=p8_table();for(int i=1;i<=6;i++)p8_set(t,p8_int(i),p8_int(i));int cursor=0,seen=0;V last=p8_nil(),key,value;while(p8_iter_next("all",t,&cursor,&last,&key,&value)){seen++;if((value.n/65536)%2){V args[]={t,value};p8_api("del",2,args);}}CHECK(seen==6);CHECK(p8_unary("#",t).n==3*65536);CHECK(p8_get(t,p8_int(1)).n==2*65536);
V splitargs[]={p8_str("1,,2," )};V s=p8_api("split",1,splitargs);CHECK(p8_unary("#",s).n==4*65536);CHECK(p8_get(s,p8_int(1)).n==65536);CHECK(p8_get(s,p8_int(2)).tag==P8_STR);CHECK(p8_get(s,p8_int(4)).tag==P8_STR);
V args[]={p8_num((int32_t)0x80000000),p8_int(1)};CHECK(p8_api("lshr",2,args).n==0x40000000);CHECK(p8_api("shr",2,args).n==(int32_t)0xc0000000);CHECK(p8_api("rotl",2,args).n==1);
V cs[]={p8_int(65),p8_int(255)};V string=p8_api("chr",2,cs);V os[]={string,p8_int(2)};CHECK(p8_api("ord",2,os).n==255*65536);
V keep=p8_table();p8_set(keep,p8_str("callback"),root);root=keep;
static char long_string[4096];for(int i=0;i<4095;i++)long_string[i]=(char)('a'+i%26);long_string[4095]=0;
V long_value=p8_str(long_string);p8_set(root,p8_str("long"),long_value);V joined=p8_binary("..",long_value,long_value);CHECK(p8_unary("#",joined).n==8190*65536);p8_set(root,p8_str("joined"),joined);
for(int f=0;f<100;f++){p8_begin_frame(0);char garbage[80];snprintf(garbage,sizeof garbage,"garbage-%d",f);p8_str(garbage);p8_end_frame();CHECK(p8_unary("#",p8_get(root,p8_str("long"))).n==4095*65536);}
V sub_args[]={p8_get(root,p8_str("joined")),p8_int(4096)};V tail=p8_api("sub",2,sub_args);CHECK(p8_binary("==",tail,long_value).n==1);
CHECK(p8_api("tostr",1,&long_value).n==long_value.n);
#ifdef P8_API_PROFILE
p8_profile_write("api-profile.json");
#endif
puts("RUNTIME_CLOSURE_ITERATOR_OK frames=2100 multi_return=ok string_arena=ok");return 0;}
