/* Pocket AOT: bounded dynamic values and an initial PICO-8 API subset. */
#include "p8.h"
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <limits.h>
#include "font.h"

#define NT 512
#define NE 3072
#define NS 256
#define SL (P8_MAX_STRING+1)
#define STRING_BYTES 32768
#define NC 1024
#define NF 256
#define NU 2048
#define MAX_CAPTURE 64
static struct {uint16_t head;uint8_t used,mark;} tables[NT];
static struct {V key,value;uint16_t next;uint8_t used;} entries[NE];
static struct {uint16_t offset,length;uint8_t used,mark;} strings[NS];
static char string_data[STRING_BYTES],string_scratch[SL];
static struct {V value;uint8_t used,mark;} cells[NC];
static struct {int fn;uint16_t head,count;uint8_t used,mark;} closures[NF];
static struct {V cell;uint16_t next;uint8_t used;} captures[NU];
static V result_values[P8_MAX_RESULTS];
static int result_count;
#ifdef P8_API_PROFILE
static uint64_t api_profile_hits[P8_API_COUNT], iterator_profile_hits[3];
#endif
extern const uint8_t p8_cart_gfx[8192],p8_cart_map[4096],p8_cart_flags[256];
static uint8_t gfx[8192],map_data[4096],flags[256];
uint8_t game_frame[128*128],game_buttons;
const uint8_t pico_palette[16][3]={
 {0,0,0},{29,43,83},{126,37,83},{0,135,81},{171,82,54},{95,87,79},{194,195,199},{255,241,232},
 {255,0,77},{255,163,0},{255,236,39},{0,228,54},{41,173,255},{131,118,156},{255,119,168},{255,204,170}};
jmp_buf p8_guard;
char p8_error[128];
uint32_t p8_frames,p8_operations,p8_heap_entries,p8_heap_strings;
static unsigned budget,depth;
static uint8_t palette[16],transparent[16],previous,pressed;
static uint16_t held_frames[6];
static int camera_x,camera_y,clip_x0,clip_y0,clip_x1,clip_y1,draw_color,cursor_x,cursor_y;
static uint32_t random_state;
static int line_x,line_y;
static bool line_started;
static int screen_bound(int n){return n<0?0:n>128?128:n;}

void p8_fail(const char *s){snprintf(p8_error,sizeof p8_error,"%s",s);longjmp(p8_guard,1);}
void p8_tick(void){if(!budget--)p8_fail("frame instruction budget exceeded");p8_operations++;}
__attribute__((weak)) void p8_stack_check(void){}
void p8_enter(void){p8_stack_check();p8_tick();if(++depth>32)p8_fail("call depth exceeds 32");}
void p8_leave(void){if(depth)depth--;}
bool p8_truth(V v){return v.tag!=P8_NIL&&(v.tag!=P8_BOOL||v.n);}
int32_t p8_number(V v){if(v.tag!=P8_NUM)p8_fail("number expected");return v.n;}
static int integer(V v){return p8_number(v)>>16;}
static bool equal(V a,V b){return a.tag==b.tag&&a.n==b.n;}
static const char *string(V v){if(v.tag!=P8_STR||(unsigned)v.n>=NS||!strings[v.n].used)p8_fail("string expected");return string_data+strings[v.n].offset;}
const char *p8_describe(V v,char *out,size_t n){switch(v.tag){
 case P8_NUM:if(!(v.n&65535))snprintf(out,n,"%d",v.n/65536);else snprintf(out,n,"%.4f",(double)v.n/65536.0);break;
 case P8_STR:snprintf(out,n,"%s",string(v));break;case P8_BOOL:snprintf(out,n,"%s",v.n?"true":"false");break;
 case P8_NIL:snprintf(out,n,"nil");break;case P8_FUNCTION:snprintf(out,n,"function");break;default:snprintf(out,n,"table");}return out;}
static V string_bytes(const char *s,size_t len){
 if(len>=SL)p8_fail("string exceeds 8191 bytes");
 for(int i=0;i<NS;i++)if(strings[i].used&&strings[i].length==len&&!memcmp(string_data+strings[i].offset,s,len))return(V){i,P8_STR};
 int id=-1;for(int i=0;i<NS;i++)if(!strings[i].used){id=i;break;}
 if(id<0)p8_fail("string slots exhausted in one frame");
 size_t offset=0;
 /* Find a free interval without moving strings still held by C temporaries. */
 for(;;){bool moved=false;for(int i=0;i<NS;i++)if(strings[i].used){size_t begin=strings[i].offset,end=begin+strings[i].length+1;if(offset<end&&offset+len+1>begin){offset=end;moved=true;break;}}if(!moved)break;}
 if(offset+len+1>STRING_BYTES)p8_fail("32KB string arena exhausted in one frame");
 memcpy(string_data+offset,s,len);string_data[offset+len]=0;
 strings[id].offset=(uint16_t)offset;strings[id].length=(uint16_t)len;strings[id].used=1;strings[id].mark=0;return(V){id,P8_STR};
}
V p8_str(const char *s){return string_bytes(s,strlen(s));}
static int table_id(V t){if(t.tag!=P8_TABLE||(unsigned)t.n>=NT||!tables[t.n].used)p8_fail("table expected");return t.n;}
V p8_table(void){for(int i=0;i<NT;i++)if(!tables[i].used){tables[i].used=1;tables[i].head=0;return(V){i,P8_TABLE};}p8_fail("table pool exhausted");return p8_nil();}
V p8_get(V t,V key){int id=table_id(t);for(int at=tables[id].head;at;at=entries[at-1].next){if(equal(entries[at-1].key,key))return entries[at-1].value;}return p8_nil();}
void p8_set(V t,V key,V value){int id=table_id(t);if(key.tag==P8_NIL)p8_fail("nil table key");uint16_t *link=&tables[id].head;
 while(*link){int i=*link-1;if(equal(entries[i].key,key)){if(value.tag==P8_NIL){*link=entries[i].next;entries[i].used=0;}else entries[i].value=value;return;}link=&entries[i].next;}
 if(value.tag==P8_NIL)return;
 for(int i=0;i<NE;i++)if(!entries[i].used){entries[i].used=1;entries[i].key=key;entries[i].value=value;entries[i].next=tables[id].head;tables[id].head=i+1;return;}p8_fail("table entries exhausted");}
static int length(V t){if(t.tag==P8_STR)return strlen(string(t));table_id(t);int n=0;while(n<NE&&p8_get(t,p8_int(n+1)).tag!=P8_NIL)n++;return n;}
static int cell_id(V v){if(v.tag!=P8_CELL||(unsigned)v.n>=NC||!cells[v.n].used)p8_fail("invalid lexical cell");return v.n;}
static int closure_id(V v){if(v.tag!=P8_FUNCTION||(unsigned)v.n>=NF||!closures[v.n].used)p8_fail("function expected");return v.n;}
V p8_cell(V initial){for(int i=0;i<NC;i++)if(!cells[i].used){cells[i].used=1;cells[i].mark=0;cells[i].value=initial;return(V){i,P8_CELL};}p8_fail("lexical cell pool exhausted in one frame");return p8_nil();}
V p8_cell_get(V v){return cells[cell_id(v)].value;}
void p8_cell_set(V v,V value){cells[cell_id(v)].value=value;}
V p8_closure(int fn,int count,const V *refs){
 if(count<0||count>MAX_CAPTURE)p8_fail("closure exceeds 64 captures");
 int id=-1,free_count=0;for(int i=0;i<NF;i++)if(!closures[i].used){id=i;break;}
 if(id<0)p8_fail("native closure pool exhausted in one frame");
 for(int i=0;i<NU;i++)free_count+=!captures[i].used;
 if(free_count<count)p8_fail("closure capture pool exhausted in one frame");
 closures[id].used=1;closures[id].mark=0;closures[id].fn=fn;closures[id].count=count;closures[id].head=0;
 uint16_t *link=&closures[id].head;int at=0;
 for(int n=0;n<count;n++){
  cell_id(refs[n]);while(captures[at].used)at++;
  captures[at].used=1;captures[at].cell=refs[n];captures[at].next=0;*link=at+1;link=&captures[at].next;
 }
 return(V){id,P8_FUNCTION};
}
V p8_capture(V self,int index){int id=closure_id(self);if(index<0||index>=closures[id].count)p8_fail("invalid closure capture");int at=closures[id].head;while(index--)at=captures[at-1].next;return captures[at-1].cell;}
/* Existing generated fixtures have no closures; new compilers override this. */
__attribute__((weak)) V p8_dispatch(int fn,V self,int argc,const V *argv){(void)fn;(void)self;(void)argc;(void)argv;p8_fail("native closure dispatcher missing");return p8_nil();}
V p8_call(V function,int argc,const V *argv){int id=closure_id(function);if(argc<0)p8_fail("invalid call argument count");p8_enter();V result=p8_dispatch(closures[id].fn,function,argc,argv);p8_leave();return result;}
V p8_return(int count,const V *values){if(count<0||count>P8_MAX_RESULTS)p8_fail("native return exceeds 16 results");if(count)memmove(result_values,values,(size_t)count*sizeof(V));result_count=count;return count?result_values[0]:p8_nil();}
int p8_result_count(void){return result_count;}
V p8_result(int index){return index<0||index>=result_count?p8_nil():result_values[index];}
bool p8_iter_next(const char *kind,V t,int *cursor,V *last,V *key,V *value){
#ifdef P8_API_PROFILE
 if(!strcmp(kind,"all"))iterator_profile_hits[0]++;else if(!strcmp(kind,"pairs"))iterator_profile_hits[1]++;else if(!strcmp(kind,"ipairs"))iterator_profile_hits[2]++;
#endif
 p8_tick();int id=table_id(t);
 if(!strcmp(kind,"all")||!strcmp(kind,"ipairs")){
  int i=*cursor;
  /* DEL shifts array entries left. Revisit this slot if its value changed. */
  if(!strcmp(kind,"all")&&i>0&&!equal(p8_get(t,p8_int(i)),*last)){}else i++;
  V v=p8_get(t,p8_int(i));if(v.tag==P8_NIL)return false;
  *cursor=i;*last=v;*key=p8_int(i);*value=v;return true;
 }
 if(!strcmp(kind,"pairs")){
  /* Pool slot order is unspecified, as with Lua's hash traversal. */
  for(int i=*cursor;i<NE;i++)if(entries[i].used){
   bool member=false;for(int at=tables[id].head;at;at=entries[at-1].next)if(at==i+1){member=true;break;}
   if(member){*cursor=i+1;*key=entries[i].key;*value=entries[i].value;*last=*key;return true;}
  }
  return false;
 }
 p8_fail("unsupported generic iterator");return false;
}
static unsigned mark_depth;
void p8_mark(V v){
 if(v.tag==P8_STR){if((unsigned)v.n>=NS||!strings[v.n].used)p8_fail("bad string reference");strings[v.n].mark=1;return;}
 if(v.tag==P8_TABLE){int id=table_id(v);if(tables[id].mark)return;tables[id].mark=1;}
 else if(v.tag==P8_CELL){int id=cell_id(v);if(cells[id].mark)return;cells[id].mark=1;}
 else if(v.tag==P8_FUNCTION){int id=closure_id(v);if(closures[id].mark)return;closures[id].mark=1;}
 else return;
 if(++mark_depth>128)p8_fail("object graph exceeds GC depth limit");
 if(v.tag==P8_TABLE){for(int at=tables[v.n].head;at;at=entries[at-1].next){p8_mark(entries[at-1].key);p8_mark(entries[at-1].value);}}
 else if(v.tag==P8_CELL)p8_mark(cells[v.n].value);
 else for(int at=closures[v.n].head;at;at=captures[at-1].next)p8_mark(captures[at-1].cell);
 mark_depth--;
}
void p8_collect(void){mark_depth=0;for(int i=0;i<NT;i++)tables[i].mark=0;for(int i=0;i<NS;i++)strings[i].mark=0;for(int i=0;i<NC;i++)cells[i].mark=0;for(int i=0;i<NF;i++)closures[i].mark=0;p8_program_roots();
 for(int i=0;i<NT;i++)if(tables[i].used&&!tables[i].mark){int at=tables[i].head;while(at){int next=entries[at-1].next;entries[at-1].used=0;at=next;}tables[i].used=0;}
 for(int i=0;i<NC;i++)if(cells[i].used&&!cells[i].mark)cells[i].used=0;
 for(int i=0;i<NF;i++)if(closures[i].used&&!closures[i].mark){int at=closures[i].head;while(at){int next=captures[at-1].next;captures[at-1].used=0;at=next;}closures[i].used=0;}
 p8_heap_strings=p8_heap_entries=0;uint16_t order[NS];int count=0;
 for(int i=0;i<NS;i++){if(strings[i].used&&!strings[i].mark)strings[i].used=0;if(strings[i].used){int at=count;while(at&&strings[order[at-1]].offset>strings[i].offset){order[at]=order[at-1];at--;}order[at]=i;count++;}p8_heap_strings+=strings[i].used;}
 size_t offset=0;for(int n=0;n<count;n++){int id=order[n];size_t len=strings[id].length+1;memmove(string_data+offset,string_data+strings[id].offset,len);strings[id].offset=(uint16_t)offset;offset+=len;}
 for(int i=0;i<NE;i++)p8_heap_entries+=entries[i].used;}
V p8_unary(const char *op,V a){if(!strcmp(op,"not"))return p8_bool(!p8_truth(a));if(!strcmp(op,"#"))return p8_int(length(a));if(!strcmp(op,"-"))return p8_num((int32_t)(0u-(uint32_t)p8_number(a)));if(!strcmp(op,"~"))return p8_num(~p8_number(a));p8_fail("unknown unary operator");return p8_nil();}
static int64_t floor_div(int64_t a,int64_t b){int64_t q=a/b,r=a%b;return q-((r!=0)&&((r<0)!=(b<0)));}
V p8_binary(const char *op,V a,V b){
 if(!strcmp(op,"=="))return p8_bool(equal(a,b));if(!strcmp(op,"~=")||!strcmp(op,"!="))return p8_bool(!equal(a,b));
 if(!strcmp(op,"..")){if((a.tag!=P8_STR&&a.tag!=P8_NUM)||(b.tag!=P8_STR&&b.tag!=P8_NUM))p8_fail("concatenation requires strings or numbers");char sa[64],sb[64];const char *as=a.tag==P8_STR?string(a):p8_describe(a,sa,sizeof sa),*bs=b.tag==P8_STR?string(b):p8_describe(b,sb,sizeof sb);size_t alen=strlen(as),blen=strlen(bs);if(alen+blen>=SL)p8_fail("concatenated string too long");memcpy(string_scratch,as,alen);memcpy(string_scratch+alen,bs,blen+1);return p8_str(string_scratch);}
 if(a.tag==P8_STR&&b.tag==P8_STR){int c=strcmp(string(a),string(b));if(!strcmp(op,"<"))return p8_bool(c<0);if(!strcmp(op,">"))return p8_bool(c>0);if(!strcmp(op,"<="))return p8_bool(c<=0);if(!strcmp(op,">="))return p8_bool(c>=0);}
 int32_t x=p8_number(a),y=p8_number(b);uint32_t ux=(uint32_t)x,uy=(uint32_t)y;
 if(!strcmp(op,"+"))return p8_num((int32_t)(ux+uy));if(!strcmp(op,"-"))return p8_num((int32_t)(ux-uy));
 if(!strcmp(op,"*"))return p8_num((int32_t)floor_div((int64_t)x*y,65536));
 if(!strcmp(op,"/"))return p8_num(y?(int32_t)(((int64_t)x*65536)/y):(x<0?-INT32_MAX:INT32_MAX));
 if(!strcmp(op,"//"))return p8_num(y?(int32_t)((uint64_t)floor_div(x,y)<<16):(x<0?-INT32_MAX:INT32_MAX));
 if(!strcmp(op,"%")){if(!y)return p8_num(0);return p8_num((int32_t)((int64_t)x-floor_div(x,y)*y));}
 if(!strcmp(op,"<"))return p8_bool(x<y);if(!strcmp(op,">"))return p8_bool(x>y);if(!strcmp(op,"<="))return p8_bool(x<=y);if(!strcmp(op,">="))return p8_bool(x>=y);
 if(!strcmp(op,"&"))return p8_num(x&y);if(!strcmp(op,"|"))return p8_num(x|y);if(!strcmp(op,"^^"))return p8_num(x^y);
 if((!strcmp(op,"<<")||!strcmp(op,">>"))&&(y<0||y>31*65536))p8_fail("shift counts outside 0..31 are unsupported");
 if(!strcmp(op,"<<"))return p8_num((int32_t)(ux<<((uy>>16)&31)));if(!strcmp(op,">>"))return p8_num(x>>((uy>>16)&31));
 if(!strcmp(op,"^")){double r=pow((double)x/65536.0,(double)y/65536.0);if(!isfinite(r)||fabs(r)>32767)p8_fail("power outside supported numeric range");return p8_num((int32_t)(r*65536));}
 p8_fail("unknown binary operator");return p8_nil();}

static void pixel(int x,int y,int c){x-=camera_x;y-=camera_y;if(x>=clip_x0&&x<clip_x1&&y>=clip_y0&&y<clip_y1)game_frame[y*128+x]=palette[c&15];}
static int sample(int x,int y){if((unsigned)x>=128||(unsigned)y>=128)return 0;uint8_t p=gfx[y*64+x/2];return(p>>((x&1)*4))&15;}
static void set_sample(int x,int y,int c){if((unsigned)x>=128||(unsigned)y>=128)return;uint8_t *p=&gfx[y*64+x/2];int shift=(x&1)*4;*p=(*p&~(15<<shift))|((c&15)<<shift);}
static int mapget(int x,int y){if((unsigned)x>=128||(unsigned)y>=64)return 0;int at=y*128+x;if(at<4096)return map_data[at];return gfx[4096+(at-4096)];}
static void mapset(int x,int y,int v){if((unsigned)x>=128||(unsigned)y>=64)return;int at=y*128+x;if(at<4096)map_data[at]=v;else gfx[at]=v;}
static void rectfill(int x,int y,int xx,int yy,int c){if(x>xx){int t=x;x=xx;xx=t;}if(y>yy){int t=y;y=yy;yy=t;}x-=camera_x;xx-=camera_x;y-=camera_y;yy-=camera_y;
 if(x<clip_x0)x=clip_x0;if(y<clip_y0)y=clip_y0;if(xx>=clip_x1)xx=clip_x1-1;if(yy>=clip_y1)yy=clip_y1-1;
 for(int j=y;j<=yy;j++)if(x<=xx)memset(game_frame+j*128+x,palette[c&15],xx-x+1);}
static void line_draw(int x,int y,int xx,int yy,int c){if(abs(xx-x)>131072||abs(yy-y)>131072)p8_fail("line too long");int dx=abs(xx-x),sx=x<xx?1:-1,dy=-abs(yy-y),sy=y<yy?1:-1,err=dx+dy;
 for(;;){pixel(x,y,c);if(x==xx&&y==yy)break;int e=2*err;if(e>=dy){err+=dy;x+=sx;}if(e<=dx){err+=dx;y+=sy;}}}
static void circle(int x,int y,int r,int c,bool fill){if(r<0)return;if(r>2048)p8_fail("circle radius exceeds subset limit");int xx=r,yy=0,err=1-r;
 while(xx>=yy){if(fill){rectfill(x-xx,y+yy,x+xx,y+yy,c);rectfill(x-xx,y-yy,x+xx,y-yy,c);rectfill(x-yy,y+xx,x+yy,y+xx,c);rectfill(x-yy,y-xx,x+yy,y-xx,c);}
 else{pixel(x+xx,y+yy,c);pixel(x-xx,y+yy,c);pixel(x+xx,y-yy,c);pixel(x-xx,y-yy,c);pixel(x+yy,y+xx,c);pixel(x-yy,y+xx,c);pixel(x+yy,y-xx,c);pixel(x-yy,y-xx,c);}yy++;if(err<0)err+=2*yy+1;else{xx--;err+=2*(yy-xx)+1;}}}
void p8_text(const char *s,int x,int y,int c){int origin=x;for(;*s;s++,x+=4){unsigned ch=(uint8_t)*s;if(ch=='\n'){x=origin-4;y+=6;continue;}
 // Source strings use P8SCII bytes. Common button icons match the physical A/B labels.
 if(ch>=128){if(ch==142)ch='a';else if(ch==151)ch='b';else ch='?';}
 ch&=127;for(int row=0;row<6;row++)for(int col=0;col<4;col++)if(p8_font[ch*6+row]&(1<<col))pixel(x+col,y+row,c);}}
static void sprite(int n,int x,int y,int w,int h,bool fx,bool fy){int sx=(n%16)*8,sy=(n/16)*8;int width=w*8,height=h*8;if(width>1024||height>1024||width<0||height<0)p8_fail("sprite dimensions outside supported range");
 for(int yy=0;yy<height;yy++)for(int xx=0;xx<width;xx++){int c=sample(sx+(fx?width-1-xx:xx),sy+(fy?height-1-yy:yy));if(!transparent[c])pixel(x+xx,y+yy,c);}}
static double real(V v){return (double)p8_number(v)/65536.0;}
static V numeric(double n){if(!isfinite(n)||n< -32768.0||n>=32768.0)p8_fail("math result outside fixed point range");return p8_num((int32_t)floor(n*65536.));}
static uint32_t random_next(void){uint32_t x=random_state;x^=x<<13;x^=x>>17;x^=x<<5;return random_state=x?x:1;}
#define HAS(i) ((i)<argc)
#define A(i) (HAS(i)?argv[i]:p8_nil())
#define I(i,def) (HAS(i)?integer(argv[i]):(def))
#define C(i) I(i,draw_color)
#define IS(api) (id==(api))
V p8_api_id(int id,int argc,const V *argv){
#ifdef P8_API_PROFILE
 if((unsigned)id<P8_API_COUNT)api_profile_hits[id]++;
#endif
 p8_tick();
 if(IS(P8_API_cls)){memset(game_frame,palette[I(0,0)&15],sizeof game_frame);cursor_x=cursor_y=clip_x0=clip_y0=0;clip_x1=clip_y1=128;return p8_nil();}
 if(IS(P8_API_pset)){pixel(I(0,0),I(1,0),C(2));return p8_nil();}
 if(IS(P8_API_pget)){int x=I(0,0)-camera_x,y=I(1,0)-camera_y;return p8_int((unsigned)x<128&&(unsigned)y<128?game_frame[y*128+x]:0);}
 if(IS(P8_API_rectfill)||IS(P8_API_rect)){int x=I(0,0),y=I(1,0),xx=I(2,0),yy=I(3,0),c=C(4);draw_color=c&15;if(IS(P8_API_rectfill))rectfill(x,y,xx,yy,c);else{line_draw(x,y,xx,y,c);line_draw(xx,y,xx,yy,c);line_draw(xx,yy,x,yy,c);line_draw(x,yy,x,y,c);}return p8_nil();}
 if(IS(P8_API_line)){
  if(argc>5)p8_fail("line accepts at most five arguments");
  if(argc<2){if(argc)draw_color=I(0,6)&15;line_started=false;return p8_nil();}
  int x=I(0,0),y=I(1,0),c=argc<4?C(2):C(4);draw_color=c&15;
  if(argc>=4){line_x=x;line_y=y;x=I(2,0);y=I(3,0);line_started=true;}
  if(line_started)line_draw(line_x,line_y,x,y,c);
  line_x=x;line_y=y;line_started=true;return p8_nil();
 }
 if(IS(P8_API_circfill)||IS(P8_API_circ)){int c=C(3);draw_color=c&15;circle(I(0,0),I(1,0),I(2,4),c,IS(P8_API_circfill));return p8_nil();}
 if(IS(P8_API_print)){
  char description[128];const char *s=A(0).tag==P8_STR?string(A(0)):p8_describe(A(0),description,sizeof description);bool implicit=argc<=2;
  int x=implicit?cursor_x:I(1,0),y=implicit?cursor_y:I(2,0),c=implicit?C(1):C(3),lines=1;
  for(const char *at=s;*at;at++)if(*at=='\n')lines++;
  if(implicit&&y+(lines-1)*6>122){int scroll=((y+(lines-1)*6-122+5)/6)*6;
   if(scroll>=128)memset(game_frame,0,sizeof game_frame);
   else{memmove(game_frame,game_frame+scroll*128,(128-scroll)*128);memset(game_frame+(128-scroll)*128,0,scroll*128);}
   y-=scroll;
  }
  draw_color=c&15;p8_text(s,x,y,c);cursor_x=0;cursor_y=y+6*lines;return p8_int(x+4*(int)strlen(s));
 }
 if(IS(P8_API_color)){int old=draw_color;draw_color=I(0,6)&15;return p8_int(old);}
 if(IS(P8_API_cursor)){V old[]={p8_int(cursor_x),p8_int(cursor_y)};cursor_x=I(0,0);cursor_y=I(1,0);if(argc>2)draw_color=I(2,6)&15;return p8_return(2,old);}
 if(IS(P8_API_camera)){V old[]={p8_int(camera_x),p8_int(camera_y)};camera_x=I(0,0);camera_y=I(1,0);return p8_return(2,old);}
 if(IS(P8_API_clip)){
  V old[]={p8_int(clip_x0),p8_int(clip_y0),p8_int(clip_x1-clip_x0),p8_int(clip_y1-clip_y0)};
  int x=I(0,0),y=I(1,0),x1=screen_bound(x+I(2,128)),y1=screen_bound(y+I(3,128));
  x=screen_bound(x);y=screen_bound(y);
  if(p8_truth(A(4))){if(x<clip_x0)x=clip_x0;if(y<clip_y0)y=clip_y0;if(x1>clip_x1)x1=clip_x1;if(y1>clip_y1)y1=clip_y1;}
  clip_x0=x;clip_y0=y;clip_x1=x1<x?x:x1;clip_y1=y1<y?y:y1;
  return p8_return(4,old);
 }
 if(IS(P8_API_pal)){if(!argc){for(int i=0;i<16;i++)palette[i]=i;return p8_nil();}if(I(2,0)!=0)p8_fail("screen palette remapping is not supported yet");palette[I(0,0)&15]=I(1,0)&15;return p8_nil();}
 if(IS(P8_API_palt)){
  unsigned old=0;for(int i=0;i<16;i++)old|=(unsigned)transparent[i]<<(15-i);
  if(!argc){for(int i=0;i<16;i++)transparent[i]=(i==0);}
  else if(argc==1){unsigned mask=(unsigned)I(0,0)&65535;for(int i=0;i<16;i++)transparent[i]=(mask>>(15-i))&1;}
  else transparent[I(0,0)&15]=p8_truth(A(1));
  return p8_int((int)old);
 }
 if(IS(P8_API_spr)){sprite(I(0,0),I(1,0),I(2,0),I(3,1),I(4,1),p8_truth(A(5)),p8_truth(A(6)));return p8_nil();}
 if(IS(P8_API_sspr)){int sx=I(0,0),sy=I(1,0),sw=I(2,1),sh=I(3,1),dx=I(4,0),dy=I(5,0),dw=I(6,sw),dh=I(7,sh);bool fx=p8_truth(A(8)),fy=p8_truth(A(9));if(sw<1||sh<1||dw<1||dh<1||dw>1024||dh>1024)p8_fail("unsupported sspr dimensions");for(int y=0;y<dh;y++)for(int x=0;x<dw;x++){int c=sample(sx+(fx?dw-1-x:x)*sw/dw,sy+(fy?dh-1-y:y)*sh/dh);if(!transparent[c])pixel(dx+x,dy+y,c);}return p8_nil();}
 if(IS(P8_API_sget))return p8_int(sample(I(0,0),I(1,0)));
 if(IS(P8_API_sset)){set_sample(I(0,0),I(1,0),C(2));return p8_nil();}
 if(IS(P8_API_mget))return p8_int(mapget(I(0,0),I(1,0)));
 if(IS(P8_API_mset)){mapset(I(0,0),I(1,0),I(2,0));return p8_nil();}
 if(IS(P8_API_fget)){int f=flags[I(0,0)&255];return argc>1?p8_bool((f>>(I(1,0)&7))&1):p8_int(f);}
 if(IS(P8_API_fset)){int i=I(0,0)&255;if(argc==2)flags[i]=I(1,0);else if(p8_truth(A(2)))flags[i]|=1<<(I(1,0)&7);else flags[i]&=~(1<<(I(1,0)&7));return p8_nil();}
 if(IS(P8_API_map)){int mx=I(0,0),my=I(1,0),dx=I(2,0),dy=I(3,0),w=I(4,128),h=I(5,32),mask=I(6,0);if(w>128||h>64)p8_fail("map dimensions outside supported range");for(int y=0;y<h;y++)for(int x=0;x<w;x++){int tile=mapget(mx+x,my+y);if(tile&&(!mask||(flags[tile]&mask)))sprite(tile,dx+x*8,dy+y*8,1,1,false,false);}return p8_nil();}
 if(IS(P8_API_btn)||IS(P8_API_btnp)){if(I(1,0)!=0)p8_fail("only player one is supported");unsigned bits=IS(P8_API_btn)?game_buttons:pressed;if(!argc)return p8_int(bits);int b=I(0,0);return p8_bool((unsigned)b<6&&(bits&(1u<<b)));}
 if(IS(P8_API_rawget)){if(argc!=2)p8_fail("rawget requires two arguments");return p8_get(A(0),A(1));}
 if(IS(P8_API_rawset)){if(argc!=3)p8_fail("rawset requires three arguments");p8_set(A(0),A(1),A(2));return A(0);}
 if(IS(P8_API_rawequal)){if(argc!=2)p8_fail("rawequal requires two arguments");return p8_bool(equal(A(0),A(1)));}
 if(IS(P8_API_rawlen)){if(argc!=1)p8_fail("rawlen requires one argument");return p8_int(length(A(0)));}
 if(IS(P8_API_select)){
  if(argc<1||argc>P8_MAX_RESULTS)p8_fail("select requires 1 to 16 arguments");
  if(A(0).tag==P8_STR&&string(A(0))[0]=='#'){V n=p8_int(argc-1);return p8_return(1,&n);}
  int index=p8_number(A(0))/65536;
  if(index<0)index=argc+index;else if(index>argc)index=argc;
  if(index<1)p8_fail("select index out of range");
  return p8_return(argc-index,argv+index);
 }
 if(IS(P8_API_add)){V t=A(0),v=A(1);int len=length(t),i=I(2,len+1);if(i<1||i>len+1)p8_fail("invalid add index");for(int j=len;j>=i;j--)p8_set(t,p8_int(j+1),p8_get(t,p8_int(j)));p8_set(t,p8_int(i),v);return v;}
 if(IS(P8_API_del)||IS(P8_API_deli)){V t=A(0);int len=length(t),index=IS(P8_API_deli)?I(1,len):0;if(IS(P8_API_del))for(int i=1;i<=len;i++)if(equal(p8_get(t,p8_int(i)),A(1))){index=i;break;}if(index<1||index>len)return p8_nil();V old=p8_get(t,p8_int(index));for(int i=index;i<len;i++)p8_set(t,p8_int(i),p8_get(t,p8_int(i+1)));p8_set(t,p8_int(len),p8_nil());return old;}
 if(IS(P8_API_count)){V t=A(0);int len=length(t);if(argc<2)return p8_int(len);int n=0;for(int i=1;i<=len;i++)if(equal(p8_get(t,p8_int(i)),A(1)))n++;return p8_int(n);}
 if(IS(P8_API_foreach)){int cursor=0;V last=p8_nil(),key,value;while(p8_iter_next("all",A(0),&cursor,&last,&key,&value))p8_call(A(1),1,&value);return p8_nil();}
 if(IS(P8_API_split)){
  const char *s=string(A(0));V out=p8_table();int n=0;bool convert=argc<3||p8_truth(A(2));
  int chunk=HAS(1)&&A(1).tag==P8_NUM?integer(A(1)):0;
  const char *sep=HAS(1)&&A(1).tag!=P8_NIL&&A(1).tag!=P8_NUM?string(A(1)):"\x2c";
  if(HAS(1)&&A(1).tag==P8_NUM&&chunk<1)p8_fail("split chunk length must be positive");
  if(!*sep)chunk=1;
  size_t seplen=strlen(sep);const char *at=s;
  do{
   const char *end=chunk?at+(strlen(at)<(size_t)chunk?strlen(at):(size_t)chunk):strstr(at,sep);
   if(!end)end=at+strlen(at);size_t len=(size_t)(end-at);if(len>=SL)p8_fail("split token exceeds string limit");
   V v=string_bytes(at,len);
   if(convert&&len){const char *token=string(v);char *stop;double x=strtod(token,&stop);if(stop!=token&&!*stop&&isfinite(x)&&x>=-32768&&x<32768)v=numeric(x);}
   p8_set(out,p8_int(++n),v);
   if(chunk){at=end;if(!*at)break;}else{if(!*end)break;at=end+seplen;}
  }while(true);
  return out;
 }
 if(IS(P8_API_band)||IS(P8_API_bor)||IS(P8_API_bxor)){int32_t value=p8_number(A(0));for(int i=1;i<argc;i++){int32_t other=p8_number(A(i));if(IS(P8_API_band))value&=other;else if(IS(P8_API_bor))value|=other;else value^=other;}return p8_num(value);}
 if(IS(P8_API_bnot))return p8_num(~p8_number(A(0)));
 if(IS(P8_API_shl)||IS(P8_API_shr)||IS(P8_API_lshr)||IS(P8_API_rotl)||IS(P8_API_rotr)){
  int amount=I(1,0);uint32_t bits=(uint32_t)p8_number(A(0));bool left=IS(P8_API_shl)||IS(P8_API_rotl);
  if(amount<0){amount=-amount;left=!left;}unsigned n=(unsigned)amount;
  if(IS(P8_API_rotl)||IS(P8_API_rotr)){n&=31;if(!n)return p8_num((int32_t)bits);return p8_num((int32_t)(left?((bits<<n)|(bits>>(32-n))):((bits>>n)|(bits<<(32-n)))));}
  if(n>=32)return p8_num(!left&&IS(P8_API_shr)&&((int32_t)bits<0)?-1:0);
  return p8_num((int32_t)(left?bits<<n:IS(P8_API_shr)?(uint32_t)((int32_t)bits>>n):bits>>n));
 }
 if(IS(P8_API_flr))return p8_num((int32_t)((uint32_t)p8_number(A(0))&0xffff0000u));
 if(IS(P8_API_ceil)){int32_t x=p8_number(A(0));return p8_num((int32_t)(((uint32_t)x&0xffff0000u)+((x&65535)?65536u:0)));}
 if(IS(P8_API_abs)){int32_t x=p8_number(A(0));return p8_num(x<0?(int32_t)(0u-(uint32_t)x):x);}
 if(IS(P8_API_min)||IS(P8_API_max)){int32_t a=p8_number(A(0)),b=p8_number(A(1));return p8_num(IS(P8_API_min)?(a<b?a:b):(a>b?a:b));}
 if(IS(P8_API_mid)){int32_t a=p8_number(A(0)),b=p8_number(A(1)),c=p8_number(A(2));if(a>b){int32_t t=a;a=b;b=t;}if(b>c){int32_t t=b;b=c;c=t;}return p8_num(a>b?a:b);}
 if(IS(P8_API_sgn))return p8_int(p8_number(A(0))<0?-1:1);
 if(IS(P8_API_sqrt)){double x=real(A(0));return numeric(sqrt(x<0?0:x));}
 if(IS(P8_API_sin))return numeric(-sin(real(A(0))*6.283185307179586));
 if(IS(P8_API_cos))return numeric(cos(real(A(0))*6.283185307179586));
 if(IS(P8_API_atan2)){double a=atan2(-real(A(1)),real(A(0)))/6.283185307179586;if(a<0)a+=1;return numeric(a);}
 if(IS(P8_API_srand)){random_state=(uint32_t)p8_number(A(0));if(!random_state)random_state=1;return p8_nil();}
 if(IS(P8_API_rnd)){uint32_t r=random_next();if(argc&&A(0).tag==P8_TABLE){int n=length(A(0));return n?p8_get(A(0),p8_int((int)(r%(unsigned)n)+1)):p8_nil();}int32_t limit=HAS(0)?p8_number(A(0)):65536;if(limit<0)p8_fail("negative rnd range unsupported");return p8_num((int32_t)(((uint64_t)r*(uint32_t)limit)>>32));}
 if(IS(P8_API_time)||IS(P8_API_t))return p8_num((int32_t)(((uint64_t)p8_frames*65536)/p8_target_hz));
 if(IS(P8_API_tostr)){if(argc>1&&p8_truth(A(1)))p8_fail("hex tostr overload unsupported");if(A(0).tag==P8_STR)return A(0);char s[128];return p8_str(p8_describe(A(0),s,sizeof s));}
 if(IS(P8_API_tonum)){if(A(0).tag==P8_NUM)return A(0);if(A(0).tag!=P8_STR)return p8_nil();char *end;double x=strtod(string(A(0)),&end);if(*end||end==string(A(0)))return p8_nil();return numeric(x);}
 if(IS(P8_API_type)){static const char *types[]={"nil","number","boolean","string","table","function"};return p8_str(types[A(0).tag<=P8_FUNCTION?A(0).tag:0]);}
 if(IS(P8_API_ord)){if(argc>2&&I(2,1)!=1)p8_fail("ord multiple results unsupported");if(A(0).tag!=P8_STR)return p8_nil();const char *s=string(A(0));int i=I(1,1);return i<1||i>(int)strlen(s)?p8_nil():p8_int((uint8_t)s[i-1]);}
 if(IS(P8_API_chr)){if(argc>=SL)p8_fail("chr result exceeds string limit");for(int i=0;i<argc;i++){int ch=I(i,0)&255;if(!ch)p8_fail("NUL strings unsupported");string_scratch[i]=(char)ch;}string_scratch[argc]=0;return p8_str(string_scratch);}
 if(IS(P8_API_sub)){const char *s=string(A(0));int len=strlen(s),i=I(1,1),j=HAS(2)&&A(2).tag!=P8_NUM?i:I(2,len);if(i<0)i+=len+1;if(j<0)j+=len+1;if(i<1)i=1;if(j>len)j=len;int n=j>=i?j-i+1:0;if(i>len)n=0;return string_bytes(n?s+i-1:s,(size_t)n);}
 if(IS(P8_API_sfx)||IS(P8_API_music))return p8_nil();
 if(IS(P8_API_printh)){if(argc!=1)p8_fail("printh file output unsupported");char description[128];const char *s=A(0).tag==P8_STR?string(A(0)):p8_describe(A(0),description,sizeof description);puts(s);return p8_nil();}
 if(IS(P8_API_stat)){if(I(0,-1)!=1)p8_fail("stat index unsupported");/* Native budget ratio, not an emulated PICO-8 CPU measurement. */return p8_num((int32_t)(((uint64_t)(200000-budget)*65536)/200000));}
 if(IS(P8_API_assert)){if(argc<1||argc>P8_MAX_RESULTS)p8_fail("assert requires 1 to 16 arguments");if(!p8_truth(A(0)))p8_fail(argc>1&&A(1).tag==P8_STR?string(A(1)):"cartridge assertion failed");return p8_return(argc,argv);}
 p8_fail("unimplemented API");return p8_nil();
}
#undef HAS
#undef A
#undef I
#undef C
#undef IS
static const char *const api_names[P8_API_COUNT]={
  [P8_API_rawget]="rawget",
  [P8_API_rawset]="rawset",
  [P8_API_rawequal]="rawequal",
  [P8_API_rawlen]="rawlen",
  [P8_API_select]="select",
  [P8_API_cls]="cls",
  [P8_API_pset]="pset",
  [P8_API_pget]="pget",
  [P8_API_rectfill]="rectfill",
  [P8_API_rect]="rect",
  [P8_API_line]="line",
  [P8_API_circfill]="circfill",
  [P8_API_circ]="circ",
  [P8_API_print]="print",
  [P8_API_color]="color",
  [P8_API_cursor]="cursor",
  [P8_API_camera]="camera",
  [P8_API_clip]="clip",
  [P8_API_pal]="pal",
  [P8_API_palt]="palt",
  [P8_API_spr]="spr",
  [P8_API_sspr]="sspr",
  [P8_API_sget]="sget",
  [P8_API_sset]="sset",
  [P8_API_mget]="mget",
  [P8_API_mset]="mset",
  [P8_API_fget]="fget",
  [P8_API_fset]="fset",
  [P8_API_map]="map",
  [P8_API_btn]="btn",
  [P8_API_btnp]="btnp",
  [P8_API_add]="add",
  [P8_API_del]="del",
  [P8_API_deli]="deli",
  [P8_API_count]="count",
  [P8_API_foreach]="foreach",
  [P8_API_split]="split",
  [P8_API_band]="band",
  [P8_API_bor]="bor",
  [P8_API_bxor]="bxor",
  [P8_API_bnot]="bnot",
  [P8_API_shl]="shl",
  [P8_API_shr]="shr",
  [P8_API_lshr]="lshr",
  [P8_API_rotl]="rotl",
  [P8_API_rotr]="rotr",
  [P8_API_flr]="flr",
  [P8_API_ceil]="ceil",
  [P8_API_abs]="abs",
  [P8_API_min]="min",
  [P8_API_max]="max",
  [P8_API_mid]="mid",
  [P8_API_sgn]="sgn",
  [P8_API_sqrt]="sqrt",
  [P8_API_sin]="sin",
  [P8_API_cos]="cos",
  [P8_API_atan2]="atan2",
  [P8_API_srand]="srand",
  [P8_API_rnd]="rnd",
  [P8_API_time]="time",
  [P8_API_t]="t",
  [P8_API_tostr]="tostr",
  [P8_API_tonum]="tonum",
  [P8_API_type]="type",
  [P8_API_ord]="ord",
  [P8_API_chr]="chr",
  [P8_API_sub]="sub",
  [P8_API_sfx]="sfx",
  [P8_API_music]="music",
  [P8_API_printh]="printh",
  [P8_API_stat]="stat",
  [P8_API_assert]="assert",
 };
#ifdef P8_API_PROFILE
void p8_profile_write(const char *path){
 FILE *f=fopen(path,"w");if(!f){perror("API profile");return;}
 fputs("{",f);for(int id=0;id<P8_API_COUNT;id++)fprintf(f,"%s\"%s\":%llu",id?",":"",api_names[id],(unsigned long long)api_profile_hits[id]);fprintf(f,",\"all\":%llu,\"pairs\":%llu,\"ipairs\":%llu}\n",(unsigned long long)iterator_profile_hits[0],(unsigned long long)iterator_profile_hits[1],(unsigned long long)iterator_profile_hits[2]);fclose(f);
}
#endif
V p8_api(const char *name,int argc,const V *argv){
 for(int id=0;id<P8_API_COUNT;id++)if(!strcmp(name,api_names[id]))return p8_api_id(id,argc,argv);
 p8_fail("unimplemented API");return p8_nil();
}
void p8_reset(void){memset(tables,0,sizeof tables);memset(entries,0,sizeof entries);memset(strings,0,sizeof strings);memset(cells,0,sizeof cells);memset(closures,0,sizeof closures);memset(captures,0,sizeof captures);memcpy(gfx,p8_cart_gfx,sizeof gfx);memcpy(map_data,p8_cart_map,sizeof map_data);memcpy(flags,p8_cart_flags,sizeof flags);memset(game_frame,0,sizeof game_frame);
 for(int i=0;i<16;i++){palette[i]=i;transparent[i]=(i==0);}memset(held_frames,0,sizeof held_frames);camera_x=camera_y=clip_x0=clip_y0=cursor_x=cursor_y=0;clip_x1=clip_y1=128;draw_color=6;game_buttons=previous=pressed=0;p8_frames=p8_operations=p8_heap_entries=p8_heap_strings=0;depth=0;budget=200000;random_state=1;p8_error[0]=0;result_count=0;line_x=line_y=0;line_started=false;p8_program_clear();}
void p8_begin_frame(uint8_t held){budget=200000;depth=0;result_count=0;game_buttons=held;pressed=held&~previous;for(int i=0;i<6;i++){if(held&(1u<<i)){if(held_frames[i]<65535)held_frames[i]++;if(held_frames[i]>=16&&((held_frames[i]-16)%4)==0)pressed|=1u<<i;}else held_frames[i]=0;}previous=held;}
void p8_end_frame(void){p8_collect();p8_frames++;}
