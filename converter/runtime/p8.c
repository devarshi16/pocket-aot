/* Pocket AOT: bounded dynamic values and an initial PICO-8 API subset. */
#include "p8.h"
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <limits.h>
#include "font.h"

#define NT 768
#define NE 4096
#define NS 256
#define SL (P8_MAX_STRING+1)
#define STRING_BYTES 32768
#define NC 1536
#define NF 768
#define NU 4096
#define MAX_CAPTURE 64
/* Compact pool records: a table entry is used when its key tag is not nil and a
 * capture is used when its cell index is not NO_LINK. */
#define NO_LINK 0xffff
static struct {uint16_t head,meta;uint8_t used,mark;} tables[NT];
static struct {int32_t key,value;uint16_t next;uint8_t key_tag,value_tag;} entries[NE];
static struct {uint16_t offset,length;uint8_t used,mark;} strings[NS];
static char string_data[STRING_BYTES],string_scratch[SL];
static struct {V value;uint8_t used,mark;} cells[NC];
static struct {int fn;uint16_t head,count;uint8_t used,mark;} closures[NF];
static struct {uint16_t cell,next;} captures[NU];
#define ENTRY_KEY(i) ((V){entries[i].key,entries[i].key_tag})
#define ENTRY_VALUE(i) ((V){entries[i].value,entries[i].value_tag})
#define ENTRY_USED(i) (entries[i].key_tag!=P8_NIL)
static V result_values[P8_MAX_RESULTS];
static int result_count;
void *p8_stack_top;
static void collect_now(bool compact);
static bool can_collect(void){return p8_stack_top!=NULL;}
static void coroutine_roots(void);
static void scan_other_stacks(void);
static const uint8_t *current_stack_top(void);
static void scan_coroutine(int i);
static void sweep_coroutines(void);
static int coroutine_mark(int i,bool set);
#ifdef P8_API_PROFILE
static uint64_t api_profile_hits[P8_API_COUNT], iterator_profile_hits[3];
#endif
extern const uint8_t p8_cart_gfx[8192],p8_cart_map[4096],p8_cart_flags[256];
extern const uint8_t p8_cart_sound[0x1200] __attribute__((weak));
static uint8_t gfx[8192],map_data[4096],flags[256];
/* Base-RAM regions outside the sprite/map arrays (PICO-8 addresses). */
static uint8_t sound_ram[0x1200];   /* 0x3100 music, 0x3200 sfx */
static uint8_t user_ram[0x1b00];    /* 0x4300 general use */
static uint8_t persist_ram[0x100];  /* 0x5e00 cartdata */
static uint8_t hw_ram[0x100];       /* 0x5f00 draw state and hardware registers */
uint8_t game_frame[128*128],game_buttons;
uint8_t p8_screen_palette[16];
bool p8_paused_by_cart;
const uint8_t pico_palette[16][3]={
 {0,0,0},{29,43,83},{126,37,83},{0,135,81},{171,82,54},{95,87,79},{194,195,199},{255,241,232},
 {255,0,77},{255,163,0},{255,236,39},{0,228,54},{41,173,255},{131,118,156},{255,119,168},{255,204,170}};
const uint8_t pico_palette_extended[32][3]={
 {0,0,0},{29,43,83},{126,37,83},{0,135,81},{171,82,54},{95,87,79},{194,195,199},{255,241,232},
 {255,0,77},{255,163,0},{255,236,39},{0,228,54},{41,173,255},{131,118,156},{255,119,168},{255,204,170},
 {41,24,20},{17,29,53},{66,33,54},{18,83,89},{116,47,41},{73,51,59},{162,136,121},{243,239,125},
 {190,18,80},{255,108,36},{168,231,46},{0,181,67},{6,90,181},{117,70,101},{255,110,89},{255,157,129}};
jmp_buf p8_guard;
char p8_error[128];
uint32_t p8_frames,p8_operations,p8_heap_entries,p8_heap_strings;
static unsigned budget,depth;
static uint8_t palette[16],transparent[16],secondary_palette[16],previous,pressed;
static uint16_t held_frames[6];
static int camera_x,camera_y,clip_x0,clip_y0,clip_x1,clip_y1,draw_color,cursor_x,cursor_y;
static uint32_t random_state;
static int line_x,line_y;
static bool line_started;
static uint16_t fill_pattern;
static bool fill_transparent;
static bool cart_data_open;
static struct {V label,callback;} menu_items[5];
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
/* Strings carry explicit lengths, so embedded NUL bytes are preserved. */
static int string_length(V v){string(v);return strings[v.n].length;}
static bool meta_call1(V t,const char *event,V *result);
const char *p8_describe(V v,char *out,size_t n){switch(v.tag){
 case P8_NUM:if(!(v.n&65535))snprintf(out,n,"%d",v.n/65536);else{
  /* Four significant decimals, trailing zeros removed, as PICO-8 prints. */
  snprintf(out,n,"%.4f",(double)v.n/65536.0);char *end=out+strlen(out);while(end>out&&end[-1]=='0')*--end=0;if(end>out&&end[-1]=='.')*--end=0;}break;
 case P8_STR:snprintf(out,n,"%s",string(v));break;case P8_BOOL:snprintf(out,n,"%s",v.n?"true":"false");break;
 case P8_NIL:snprintf(out,n,"nil");break;case P8_FUNCTION:snprintf(out,n,"function");break;
 case P8_THREAD:snprintf(out,n,"thread");break;
 default:{V text;if(v.tag==P8_TABLE&&meta_call1(v,"__tostring",&text)&&text.tag==P8_STR){snprintf(out,n,"%s",string(text));break;}snprintf(out,n,"table");}}return out;}
static int string_slot(void){for(int i=0;i<NS;i++)if(!strings[i].used)return i;return -1;}
static bool string_place(size_t len,size_t *at){
 size_t offset=0;
 /* Find a free interval without moving strings still held by C temporaries. */
 for(;;){bool moved=false;for(int i=0;i<NS;i++)if(strings[i].used){size_t begin=strings[i].offset,end=begin+strings[i].length+1;if(offset<end&&offset+len+1>begin){offset=end;moved=true;break;}}if(!moved)break;}
 *at=offset;return offset+len+1<=STRING_BYTES;
}
static V string_bytes(const char *s,size_t len){
 if(len>=SL)p8_fail("string exceeds 8191 bytes");
 for(int i=0;i<NS;i++)if(strings[i].used&&strings[i].length==len&&!memcmp(string_data+strings[i].offset,s,len))return(V){i,P8_STR};
 /* The source bytes may live in the arena; copy them before a collection can free them. */
 bool inside=s>=string_data&&s<string_data+STRING_BYTES;
 if(inside){memmove(string_scratch,s,len);s=string_scratch;}
 int id=string_slot();size_t offset;bool fits=string_place(len,&offset);
 if((id<0||!fits)&&can_collect()){collect_now(false);id=string_slot();fits=string_place(len,&offset);}
 if(id<0)p8_fail("string slots exhausted in one frame");
 if(!fits)p8_fail("32KB string arena exhausted in one frame");
 memmove(string_data+offset,s,len);string_data[offset+len]=0;
 strings[id].offset=(uint16_t)offset;strings[id].length=(uint16_t)len;strings[id].used=1;strings[id].mark=0;return(V){id,P8_STR};
}
V p8_str(const char *s){return string_bytes(s,strlen(s));}
V p8_strn(const char *s,size_t length){return string_bytes(s,length);}
static int table_id(V t){if(t.tag!=P8_TABLE||(unsigned)t.n>=NT||!tables[t.n].used)p8_fail("table expected");return t.n;}
static int table_slot(void){for(int i=0;i<NT;i++)if(!tables[i].used)return i;return -1;}
V p8_table(void){int i=table_slot();if(i<0&&can_collect()){collect_now(false);i=table_slot();}if(i<0)p8_fail("table pool exhausted");tables[i].used=1;tables[i].head=0;tables[i].meta=0;tables[i].mark=0;return(V){i,P8_TABLE};}
static V raw_get(V t,V key){int id=table_id(t);for(int at=tables[id].head;at;at=entries[at-1].next){if(entries[at-1].key==key.n&&entries[at-1].key_tag==key.tag)return ENTRY_VALUE(at-1);}return p8_nil();}
static int entry_slot(void){for(int i=0;i<NE;i++)if(!ENTRY_USED(i))return i;return -1;}
static void raw_set(V t,V key,V value){int id=table_id(t);if(key.tag==P8_NIL)p8_fail("nil table key");uint16_t *link=&tables[id].head;
 while(*link){int i=*link-1;if(entries[i].key==key.n&&entries[i].key_tag==key.tag){if(value.tag==P8_NIL){*link=entries[i].next;entries[i].key_tag=P8_NIL;}else{entries[i].value=value.n;entries[i].value_tag=value.tag;}return;}link=&entries[i].next;}
 if(value.tag==P8_NIL)return;
 int i=entry_slot();
 if(i<0&&can_collect()){collect_now(false);i=entry_slot();}
 if(i<0)p8_fail("table entries exhausted");
 entries[i].key=key.n;entries[i].key_tag=key.tag;entries[i].value=value.n;entries[i].value_tag=value.tag;entries[i].next=tables[id].head;tables[id].head=i+1;}
/* Metatables: the metatable is itself a pool table referenced by index+1. */
static V metatable(V t){if(t.tag!=P8_TABLE)return p8_nil();int id=table_id(t);return tables[id].meta?(V){tables[id].meta-1,P8_TABLE}:p8_nil();}
static V metamethod(V t,const char *event){V m=metatable(t);return m.tag==P8_TABLE?raw_get(m,p8_str(event)):p8_nil();}
V p8_get(V t,V key){
 if(t.tag==P8_STR){const char *text=string(t);int n=string_length(t),i=integer(key);if(i<0)i+=n+1;return string_bytes(i>=1&&i<=n?text+i-1:text,i>=1&&i<=n?1:0);}
 for(int hops=0;hops<32;hops++){
  if(t.tag!=P8_TABLE)p8_fail("table expected");
  V v=raw_get(t,key);if(v.tag!=P8_NIL||!tables[t.n].meta)return v;
  V handler=metamethod(t,"__index");
  if(handler.tag==P8_NIL)return v;
  if(handler.tag==P8_FUNCTION){V args[2]={t,key};return p8_call(handler,2,args);}
  t=handler;
 }
 p8_fail("__index chain too long");return p8_nil();}
void p8_set(V t,V key,V value){
 for(int hops=0;hops<32;hops++){
  int id=table_id(t);
  if(!tables[id].meta||raw_get(t,key).tag!=P8_NIL){raw_set(t,key,value);return;}
  V handler=metamethod(t,"__newindex");
  if(handler.tag==P8_NIL){raw_set(t,key,value);return;}
  if(handler.tag==P8_FUNCTION){V args[3]={t,key,value};p8_call(handler,3,args);return;}
  t=handler;
 }
 p8_fail("__newindex chain too long");}
static int raw_length(V t){if(t.tag==P8_STR)return string_length(t);table_id(t);int n=0;while(n<NE&&raw_get(t,p8_int(n+1)).tag!=P8_NIL)n++;return n;}
static int length(V t){if(t.tag==P8_STR)return string_length(t);int n=0;while(n<NE&&p8_get(t,p8_int(n+1)).tag!=P8_NIL)n++;return n;}
static int cell_id(V v){if(v.tag!=P8_CELL||(unsigned)v.n>=NC||!cells[v.n].used)p8_fail("invalid lexical cell");return v.n;}
static int closure_id(V v){if(v.tag!=P8_FUNCTION||(unsigned)v.n>=NF||!closures[v.n].used)p8_fail("function expected");return v.n;}
static int cell_slot(void){for(int i=0;i<NC;i++)if(!cells[i].used)return i;return -1;}
V p8_cell(V initial){int i=cell_slot();if(i<0&&can_collect()){collect_now(false);i=cell_slot();}if(i<0)p8_fail("lexical cell pool exhausted in one frame");cells[i].used=1;cells[i].mark=0;cells[i].value=initial;return(V){i,P8_CELL};}
V p8_cell_get(V v){return cells[cell_id(v)].value;}
void p8_cell_set(V v,V value){cells[cell_id(v)].value=value;}
static int closure_slot(void){for(int i=0;i<NF;i++)if(!closures[i].used)return i;return -1;}
static int free_captures(void){int n=0;for(int i=0;i<NU;i++)n+=captures[i].cell==NO_LINK;return n;}
V p8_closure(int fn,int count,const V *refs){
 if(count<0||count>MAX_CAPTURE)p8_fail("closure exceeds 64 captures");
 int id=closure_slot();
 if((id<0||free_captures()<count)&&can_collect()){collect_now(false);id=closure_slot();}
 if(id<0)p8_fail("native closure pool exhausted in one frame");
 if(free_captures()<count)p8_fail("closure capture pool exhausted in one frame");
 closures[id].used=1;closures[id].mark=0;closures[id].fn=fn;closures[id].count=count;closures[id].head=0;
 uint16_t *link=&closures[id].head;int at=0;
 for(int n=0;n<count;n++){
  cell_id(refs[n]);while(captures[at].cell!=NO_LINK)at++;
  captures[at].cell=(uint16_t)refs[n].n;captures[at].next=0;*link=at+1;link=&captures[at].next;
 }
 return(V){id,P8_FUNCTION};
}
V p8_capture(V self,int index){int id=closure_id(self);if(index<0||index>=closures[id].count)p8_fail("invalid closure capture");int at=closures[id].head;while(index--)at=captures[at-1].next;return(V){captures[at-1].cell,P8_CELL};}
/* Existing generated fixtures have no closures; new compilers override this. */
__attribute__((weak)) V p8_dispatch(int fn,V self,int argc,const V *argv){(void)fn;(void)self;(void)argc;(void)argv;p8_fail("native closure dispatcher missing");return p8_nil();}
static V runtime_native(int fn,V self,int argc,const V *argv);
#define P8_RUNTIME_NATIVE 20000
/* __call receives the table as its first argument. Kept out of line so the
 * common p8_call frame stays small on the 16 KB Pico stack. */
__attribute__((noinline)) static V call_metamethod(V function,int argc,const V *argv){
 V handler=metamethod(function,"__call");if(handler.tag!=P8_FUNCTION)p8_fail("function expected");
 if(argc>=64)p8_fail("too many call arguments");
 V args[64];args[0]=function;for(int i=0;i<argc;i++)args[i+1]=argv[i];return p8_call(handler,argc+1,args);
}
V p8_call(V function,int argc,const V *argv){
 if(argc<0)p8_fail("invalid call argument count");
 if(function.tag==P8_TABLE)return call_metamethod(function,argc,argv);
 int id=closure_id(function);p8_enter();
 V result=closures[id].fn>=P8_RUNTIME_NATIVE?runtime_native(closures[id].fn,function,argc,argv):p8_dispatch(closures[id].fn,function,argc,argv);
 p8_leave();return result;}
V p8_return(int count,const V *values){if(count<0||count>P8_MAX_RESULTS)p8_fail("native return exceeds the result limit");if(count)memmove(result_values,values,(size_t)count*sizeof(V));result_count=count;return count?result_values[0]:p8_nil();}
int p8_result_count(void){return result_count;}
V p8_result(int index){return index<0||index>=result_count?p8_nil():result_values[index];}
/* Raw pairs order: insertion order is not tracked, so walk the table's chain. */
static bool table_next(V t,V key,V *next_key,V *next_value){
 int id=table_id(t),at=tables[id].head;
 if(key.tag!=P8_NIL){while(at&&!(entries[at-1].key==key.n&&entries[at-1].key_tag==key.tag))at=entries[at-1].next;if(!at)p8_fail("invalid key to next");at=entries[at-1].next;}
 if(!at)return false;*next_key=ENTRY_KEY(at-1);*next_value=ENTRY_VALUE(at-1);return true;
}
bool p8_iter_next(const char *kind,V t,int *cursor,V *last,V *key,V *value){
#ifdef P8_API_PROFILE
 if(!strcmp(kind,"all"))iterator_profile_hits[0]++;else if(!strcmp(kind,"pairs"))iterator_profile_hits[1]++;else if(!strcmp(kind,"ipairs"))iterator_profile_hits[2]++;
#endif
 p8_tick();int id=table_id(t);
 if(!strcmp(kind,"all")||!strcmp(kind,"ipairs")){
  int i=*cursor;
  /* DEL shifts array entries left. Revisit this slot if its value changed. */
  if(!strcmp(kind,"all")&&i>0&&!equal(raw_get(t,p8_int(i)),*last)){}else i++;
  V v=!strcmp(kind,"ipairs")?p8_get(t,p8_int(i)):raw_get(t,p8_int(i));if(v.tag==P8_NIL)return false;
  *cursor=i;*last=v;*key=p8_int(i);*value=v;return true;
 }
 if(!strcmp(kind,"pairs")){
  /* The cursor is the last entry slot. Removed entries keep their link, so
   * clearing fields during traversal is safe, as in Lua. */
  int at=*cursor?entries[*cursor-1].next:tables[id].head;
  while(at&&!ENTRY_USED(at-1))at=entries[at-1].next;
  if(!at)return false;
  *cursor=at;*key=ENTRY_KEY(at-1);*value=ENTRY_VALUE(at-1);*last=*key;return true;
 }
 p8_fail("unsupported generic iterator");return false;
}
/* Tracing collector with an explicit grey stack: deep lists no longer hit a
 * recursion limit. Overflow falls back to rescanning marked objects. */
static V grey[512];
static int grey_count;
static bool grey_overflow;
static bool object_live(V v){
 switch(v.tag){
  case P8_STR:return (unsigned)v.n<NS&&strings[v.n].used;
  case P8_TABLE:return (unsigned)v.n<NT&&tables[v.n].used;
  case P8_CELL:return (unsigned)v.n<NC&&cells[v.n].used;
  case P8_FUNCTION:return (unsigned)v.n<NF&&closures[v.n].used;
  case P8_THREAD:return coroutine_mark(v.n,false)>=0;
 }
 return false;
}
static void shade(V v){
 if(v.tag==P8_STR){if(!object_live(v))p8_fail("bad string reference");strings[v.n].mark=1;return;}
 if(v.tag==P8_TABLE){int id=table_id(v);if(tables[id].mark)return;tables[id].mark=1;}
 else if(v.tag==P8_CELL){int id=cell_id(v);if(cells[id].mark)return;cells[id].mark=1;}
 else if(v.tag==P8_FUNCTION){int id=closure_id(v);if(closures[id].mark)return;closures[id].mark=1;}
 else if(v.tag==P8_THREAD){int m=coroutine_mark(v.n,false);if(m<0)p8_fail("bad coroutine reference");if(m)return;coroutine_mark(v.n,true);}
 else return;
 if(grey_count<(int)(sizeof grey/sizeof grey[0]))grey[grey_count++]=v;else grey_overflow=true;
}
static bool weak_mode(V t,bool *keys,bool *values){
 V mode=metamethod(t,"__mode");*keys=*values=false;
 if(mode.tag!=P8_STR)return false;for(const char *at=string(mode);*at;at++){if(*at=='k')*keys=true;if(*at=='v')*values=true;}return *keys||*values;
}
static void scan(V v){
 if(v.tag==P8_TABLE){
  if(tables[v.n].meta)shade((V){tables[v.n].meta-1,P8_TABLE});
  bool weak_keys=false,weak_values=false;if(tables[v.n].meta)weak_mode(v,&weak_keys,&weak_values);
  for(int at=tables[v.n].head;at;at=entries[at-1].next){
   V key=ENTRY_KEY(at-1),value=ENTRY_VALUE(at-1);
   if(!weak_keys||key.tag==P8_STR)shade(key);
   if(!weak_values||value.tag==P8_STR)shade(value);
  }
 }
 else if(v.tag==P8_CELL)shade(cells[v.n].value);
 else if(v.tag==P8_FUNCTION)for(int at=closures[v.n].head;at;at=captures[at-1].next)shade((V){captures[at-1].cell,P8_CELL});
 else if(v.tag==P8_THREAD)scan_coroutine(v.n);
}
static void drain(void){
 for(;;){
  while(grey_count)scan(grey[--grey_count]);
  if(!grey_overflow)return;
  grey_overflow=false;
  for(int i=0;i<NT;i++)if(tables[i].used&&tables[i].mark)scan((V){i,P8_TABLE});
  for(int i=0;i<NC;i++)if(cells[i].used&&cells[i].mark)scan((V){i,P8_CELL});
  for(int i=0;i<NF;i++)if(closures[i].used&&closures[i].mark)scan((V){i,P8_FUNCTION});
 }
}
void p8_mark(V v){shade(v);drain();}
/* Conservative native-stack roots: any 8-byte window that looks like a live
 * reference keeps that object. False positives only delay reclamation. */
__attribute__((no_sanitize_address,noinline)) static void scan_range(const uint8_t *low,const uint8_t *high){
 for(const volatile uint8_t *p=(const uint8_t*)(((uintptr_t)low+3)&~(uintptr_t)3);p+8<=high;p+=4){
  int32_t n=(int32_t)((uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24));
  V v={n,p[4]};if(v.tag>=P8_STR&&v.tag<=P8_THREAD&&object_live(v))shade(v);
 }
}
__attribute__((noinline)) static void scan_stack(void){
 jmp_buf registers;__builtin_unwind_init();setjmp(registers);
 /* The frame address is on the real stack even when instrumentation moves locals. */
 const uint8_t *low=(const uint8_t*)__builtin_frame_address(0);
 if((const uint8_t*)&registers<low&&(const uint8_t*)&registers>low-4096)low=(const uint8_t*)&registers;
 const uint8_t *top=current_stack_top();if(top&&top>low)scan_range(low,top);
 scan_other_stacks();
}
static void mark_roots(bool native_stack){
 grey_count=0;grey_overflow=false;
 p8_program_roots();
 for(int i=0;i<result_count;i++)shade(result_values[i]);
 for(int i=0;i<5;i++){shade(menu_items[i].label);shade(menu_items[i].callback);}
 coroutine_roots();
 if(native_stack)scan_stack();
 drain();
}
static void clear_weak_entries(void){
 for(int t=0;t<NT;t++){
  if(!tables[t].used||!tables[t].mark||!tables[t].meta)continue;
  bool keys,values;if(!weak_mode((V){t,P8_TABLE},&keys,&values))continue;
  uint16_t *link=&tables[t].head;
  while(*link){int i=*link-1;V k=ENTRY_KEY(i),v=ENTRY_VALUE(i);
   bool dead=(keys&&k.tag>=P8_TABLE&&k.tag<=P8_CELL&&object_live(k)&&!((k.tag==P8_TABLE&&tables[k.n].mark)||(k.tag==P8_FUNCTION&&closures[k.n].mark)))||
             (values&&v.tag>=P8_TABLE&&v.tag<=P8_CELL&&object_live(v)&&!((v.tag==P8_TABLE&&tables[v.n].mark)||(v.tag==P8_FUNCTION&&closures[v.n].mark)));
   if(dead){*link=entries[i].next;entries[i].key_tag=P8_NIL;}else link=&entries[i].next;}
 }
}
static void sweep(bool compact){
 clear_weak_entries();
 for(int i=0;i<NT;i++)if(tables[i].used&&!tables[i].mark){int at=tables[i].head;while(at){int next=entries[at-1].next;entries[at-1].key_tag=P8_NIL;at=next;}tables[i].used=0;}
 for(int i=0;i<NC;i++)if(cells[i].used&&!cells[i].mark)cells[i].used=0;
 for(int i=0;i<NF;i++)if(closures[i].used&&!closures[i].mark){int at=closures[i].head;while(at){int next=captures[at-1].next;captures[at-1].cell=NO_LINK;at=next;}closures[i].used=0;}
 p8_heap_strings=p8_heap_entries=0;uint16_t order[NS];int count=0;
 for(int i=0;i<NS;i++){if(strings[i].used&&!strings[i].mark)strings[i].used=0;if(strings[i].used){int at=count;while(at&&strings[order[at-1]].offset>strings[i].offset){order[at]=order[at-1];at--;}order[at]=i;count++;}p8_heap_strings+=strings[i].used;}
 /* Compaction moves bytes, so it only runs at frame boundaries where no C
  * temporary holds a string pointer. */
 if(compact){size_t offset=0;for(int n=0;n<count;n++){int id=order[n];size_t len=strings[id].length+1;memmove(string_data+offset,string_data+strings[id].offset,len);strings[id].offset=(uint16_t)offset;offset+=len;}}
 for(int i=0;i<NE;i++)p8_heap_entries+=ENTRY_USED(i);
 sweep_coroutines();
}
static void clear_marks(void){for(int i=0;i<NT;i++)tables[i].mark=0;for(int i=0;i<NS;i++)strings[i].mark=0;for(int i=0;i<NC;i++)cells[i].mark=0;for(int i=0;i<NF;i++)closures[i].mark=0;}
static bool collecting;
static void collect_now(bool compact){
 if(collecting)return;collecting=true;clear_marks();mark_roots(!compact);sweep(compact);collecting=false;
}
void p8_collect(void){
 /* Called between frames, or by backends outside generated code. If a
  * native stack is registered, still scan it: init may be running flip(). */
 if(collecting)return;collecting=true;clear_marks();mark_roots(p8_stack_top!=NULL);sweep(true);collecting=false;}
/* Metamethod helpers used by operators and tostring. */
static V meta_binary(const char *event,V a,V b,bool *found){
 V h=metamethod(a,event);if(h.tag==P8_NIL)h=metamethod(b,event);
 *found=h.tag!=P8_NIL;if(!*found)return p8_nil();V args[2]={a,b};return p8_call(h,2,args);
}
static bool meta_call1(V t,const char *event,V *result){
 if(t.tag!=P8_TABLE||!tables[t.n].meta)return false;
 V h=metamethod(t,event);if(h.tag==P8_NIL)return false;
 *result=p8_call(h,1,&t);return true;
}
V p8_unary(const char *op,V a){
 if(!strcmp(op,"not"))return p8_bool(!p8_truth(a));
 if(a.tag==P8_TABLE&&tables[table_id(a)].meta){const char *event=!strcmp(op,"#")?"__len":!strcmp(op,"-")?"__unm":!strcmp(op,"~")?"__bnot":NULL;
  if(event&&metamethod(a,event).tag!=P8_NIL){V args[2]={a,a};return p8_call(metamethod(a,event),2,args);}}
 if(!strcmp(op,"#"))return p8_int(length(a));if(!strcmp(op,"-"))return p8_num((int32_t)(0u-(uint32_t)p8_number(a)));if(!strcmp(op,"~"))return p8_num(~p8_number(a));
 /* PICO-8 memory operators: @addr, %addr and $addr. */
 if(!strcmp(op,"@")){V n=a;return p8_api_id(P8_API_peek,1,&n);}
 if(!strcmp(op,"%")){V n=a;return p8_api_id(P8_API_peek2,1,&n);}
 if(!strcmp(op,"$")){V n=a;return p8_api_id(P8_API_peek4,1,&n);}
 p8_fail("unknown unary operator");return p8_nil();}
static int64_t floor_div(int64_t a,int64_t b){int64_t q=a/b,r=a%b;return q-((r!=0)&&((r<0)!=(b<0)));}
static int compare_strings(V a,V b){int la=string_length(a),lb=string_length(b),c=memcmp(string(a),string(b),(size_t)(la<lb?la:lb));return c?c:(la<lb?-1:la>lb);}
static const char *metaevent(const char *op){
 static const char *const map[][2]={{"+","__add"},{"-","__sub"},{"*","__mul"},{"/","__div"},{"%","__mod"},{"^","__pow"},{"//","__idiv"},
  {"&","__band"},{"|","__bor"},{"^^","__bxor"},{"<<","__shl"},{">>","__shr"},{">>>","__lshr"},{"<<>","__rotl"},{">><","__rotr"},{"..","__concat"},
  {"<","__lt"},{"<=","__le"}};
 for(unsigned i=0;i<sizeof map/sizeof map[0];i++)if(!strcmp(op,map[i][0]))return map[i][1];return NULL;
}
V p8_binary(const char *op,V a,V b){
 if((a.tag==P8_TABLE&&tables[table_id(a)].meta)||(b.tag==P8_TABLE&&tables[table_id(b)].meta)){
  bool found;
  if(!strcmp(op,"==")||!strcmp(op,"~=")||!strcmp(op,"!=")){
   if(a.tag==P8_TABLE&&b.tag==P8_TABLE&&!equal(a,b)){V r=meta_binary("__eq",a,b,&found);if(found){bool e=p8_truth(r);return p8_bool(!strcmp(op,"==")?e:!e);}}
  }else if(!strcmp(op,">")||!strcmp(op,">=")){V r=meta_binary(!strcmp(op,">")?"__lt":"__le",b,a,&found);if(found)return p8_bool(p8_truth(r));}
  else{const char *event=metaevent(op);if(event){V r=meta_binary(event,a,b,&found);if(found)return (!strcmp(op,"<")||!strcmp(op,"<="))?p8_bool(p8_truth(r)):r;}}
 }
 if(!strcmp(op,"=="))return p8_bool(equal(a,b));if(!strcmp(op,"~=")||!strcmp(op,"!="))return p8_bool(!equal(a,b));
 if(!strcmp(op,"..")){if((a.tag!=P8_STR&&a.tag!=P8_NUM)||(b.tag!=P8_STR&&b.tag!=P8_NUM))p8_fail("concatenation requires strings or numbers");
  char sa[64],sb[64];const char *as=a.tag==P8_STR?string(a):p8_describe(a,sa,sizeof sa),*bs=b.tag==P8_STR?string(b):p8_describe(b,sb,sizeof sb);
  size_t alen=a.tag==P8_STR?(size_t)string_length(a):strlen(as),blen=b.tag==P8_STR?(size_t)string_length(b):strlen(bs);
  if(alen+blen>=SL)p8_fail("concatenated string too long");memmove(string_scratch,as,alen);memmove(string_scratch+alen,bs,blen);return string_bytes(string_scratch,alen+blen);}
 if(a.tag==P8_STR&&b.tag==P8_STR){int c=compare_strings(a,b);if(!strcmp(op,"<"))return p8_bool(c<0);if(!strcmp(op,">"))return p8_bool(c>0);if(!strcmp(op,"<="))return p8_bool(c<=0);if(!strcmp(op,">="))return p8_bool(c>=0);}
 int32_t x=p8_number(a),y=p8_number(b);uint32_t ux=(uint32_t)x,uy=(uint32_t)y;
 if(!strcmp(op,"+"))return p8_num((int32_t)(ux+uy));if(!strcmp(op,"-"))return p8_num((int32_t)(ux-uy));
 if(!strcmp(op,"*"))return p8_num((int32_t)floor_div((int64_t)x*y,65536));
 if(!strcmp(op,"/"))return p8_num(y?(int32_t)(((int64_t)x*65536)/y):(x<0?-INT32_MAX:INT32_MAX));
 if(!strcmp(op,"//"))return p8_num(y?(int32_t)((uint64_t)floor_div(x,y)<<16):(x<0?-INT32_MAX:INT32_MAX));
 if(!strcmp(op,"%")){if(!y)return p8_num(0);return p8_num((int32_t)((int64_t)x-floor_div(x,y)*y));}
 if(!strcmp(op,"<"))return p8_bool(x<y);if(!strcmp(op,">"))return p8_bool(x>y);if(!strcmp(op,"<="))return p8_bool(x<=y);if(!strcmp(op,">="))return p8_bool(x>=y);
 if(!strcmp(op,"&"))return p8_num(x&y);if(!strcmp(op,"|"))return p8_num(x|y);if(!strcmp(op,"^^"))return p8_num(x^y);
 /* Shift amounts use the integer part; PICO-8 shifts of 32 or more saturate. */
 if(!strcmp(op,"<<")||!strcmp(op,">>")||!strcmp(op,">>>")||!strcmp(op,"<<>")||!strcmp(op,">><")){
  int n=(int)(y>>16);bool left=op[0]=='<';
  if(!strcmp(op,"<<>")||!strcmp(op,">><")){n&=31;if(!strcmp(op,">><"))n=(32-n)&31;return p8_num(n?(int32_t)((ux<<n)|(ux>>(32-n))):x);}
  bool logical=!strcmp(op,">>>");if(n<0){n=-n;left=!left;}
  if(n>=32)return p8_num(!left&&!logical&&x<0?-1:0);
  return p8_num(left?(int32_t)(ux<<n):logical?(int32_t)(ux>>n):(x>>n));
 }
 if(!strcmp(op,"^")){double r=pow((double)x/65536.0,(double)y/65536.0);if(!isfinite(r)||fabs(r)>32767)p8_fail("power outside supported numeric range");return p8_num((int32_t)(r*65536));}
 p8_fail("unknown binary operator");return p8_nil();}
static void pixel(int x,int y,int c){x-=camera_x;y-=camera_y;if(x>=clip_x0&&x<clip_x1&&y>=clip_y0&&y<clip_y1)game_frame[y*128+x]=palette[c&15];}
/* Shape pixels honour fillp(): pattern bit 15 is the top-left pixel of each
 * 4x4 cell. Set bits use the high colour nibble, or are skipped when the
 * pattern is transparent. Coordinates are screen-space after the camera. */
static void shape_pixel(int x,int y,int c){
 x-=camera_x;y-=camera_y;
 if(x<clip_x0||x>=clip_x1||y<clip_y0||y>=clip_y1)return;
 if(fill_pattern&(0x8000>>(((y&3)<<2)|(x&3)))){if(fill_transparent)return;c=(c>>4)&15;}
 game_frame[y*128+x]=palette[c&15];
}
static int sample(int x,int y){if((unsigned)x>=128||(unsigned)y>=128)return 0;uint8_t p=gfx[y*64+x/2];return(p>>((x&1)*4))&15;}
static void set_sample(int x,int y,int c){if((unsigned)x>=128||(unsigned)y>=128)return;uint8_t *p=&gfx[y*64+x/2];int shift=(x&1)*4;*p=(*p&~(15<<shift))|((c&15)<<shift);}
static int mapget(int x,int y){if((unsigned)x>=128||(unsigned)y>=64)return 0;int at=y*128+x;if(at<4096)return map_data[at];return gfx[4096+(at-4096)];}
static void mapset(int x,int y,int v){if((unsigned)x>=128||(unsigned)y>=64)return;int at=y*128+x;if(at<4096)map_data[at]=v;else gfx[at]=v;}
static void rectfill(int x,int y,int xx,int yy,int c){if(x>xx){int t=x;x=xx;xx=t;}if(y>yy){int t=y;y=yy;yy=t;}
 if(fill_pattern){for(int j=y;j<=yy;j++){int sy=j-camera_y;if(sy<clip_y0||sy>=clip_y1)continue;int a=x,b=xx;if(a-camera_x<clip_x0)a=clip_x0+camera_x;if(b-camera_x>=clip_x1)b=clip_x1-1+camera_x;for(int i=a;i<=b;i++)shape_pixel(i,j,c);}return;}
 x-=camera_x;xx-=camera_x;y-=camera_y;yy-=camera_y;
 if(x<clip_x0)x=clip_x0;if(y<clip_y0)y=clip_y0;if(xx>=clip_x1)xx=clip_x1-1;if(yy>=clip_y1)yy=clip_y1-1;
 for(int j=y;j<=yy;j++)if(x<=xx)memset(game_frame+j*128+x,palette[c&15],xx-x+1);}
static void line_draw(int x,int y,int xx,int yy,int c){if(abs(xx-x)>131072||abs(yy-y)>131072)p8_fail("line too long");int dx=abs(xx-x),sx=x<xx?1:-1,dy=-abs(yy-y),sy=y<yy?1:-1,err=dx+dy;
 for(;;){shape_pixel(x,y,c);if(x==xx&&y==yy)break;int e=2*err;if(e>=dy){err+=dy;x+=sx;}if(e<=dx){err+=dx;y+=sy;}}}
static void circle(int x,int y,int r,int c,bool fill){if(r<0)return;if(r>2048)p8_fail("circle radius exceeds subset limit");int xx=r,yy=0,err=1-r;
 while(xx>=yy){if(fill){rectfill(x-xx,y+yy,x+xx,y+yy,c);rectfill(x-xx,y-yy,x+xx,y-yy,c);rectfill(x-yy,y+xx,x+yy,y+xx,c);rectfill(x-yy,y-xx,x+yy,y-xx,c);}
 else{shape_pixel(x+xx,y+yy,c);shape_pixel(x-xx,y+yy,c);shape_pixel(x+xx,y-yy,c);shape_pixel(x-xx,y-yy,c);shape_pixel(x+yy,y+xx,c);shape_pixel(x-yy,y+xx,c);shape_pixel(x+yy,y-xx,c);shape_pixel(x-yy,y-xx,c);}yy++;if(err<0)err+=2*yy+1;else{xx--;err+=2*(yy-xx)+1;}}}
/* Ellipse inside the inclusive box x0..x1, y0..y1 (oval/ovalfill). Each row's
 * span comes from the ellipse equation at the pixel centre; the outline is
 * every span pixel with an outside 4-neighbour, so steep sides stay joined. */
static bool oval_span(int x0,int y0,int x1,int y1,int y,int *left,int *right){
 if(y<y0||y>y1)return false;
 double cx=(x0+x1)/2.0,cy=(y0+y1)/2.0,rx=(x1-x0)/2.0+0.5,ry=(y1-y0)/2.0+0.5,dy=(y-cy)/ry,t=1-dy*dy;if(t<0)t=0;
 double half=rx*sqrt(t);int l=(int)floor(cx-half+1.0+1e-6);
 if(l<x0)l=x0;if(l>(x0+x1)/2)l=(x0+x1)/2;int r=x0+x1-l;/* symmetric about the box centre */
 *left=l;*right=r;return true;
}
static void oval(int x0,int y0,int x1,int y1,int c,bool fill){
 if(x0>x1){int t=x0;x0=x1;x1=t;}if(y0>y1){int t=y0;y0=y1;y1=t;}
 if(x1-x0>4096||y1-y0>4096)p8_fail("oval exceeds subset limit");
 for(int y=y0;y<=y1;y++){
  int l,r,ul=0,ur=-1,dl=0,dr=-1;oval_span(x0,y0,x1,y1,y,&l,&r);
  if(fill){rectfill(l,y,r,y,c);continue;}
  if(!oval_span(x0,y0,x1,y1,y-1,&ul,&ur)){ul=1;ur=0;}
  if(!oval_span(x0,y0,x1,y1,y+1,&dl,&dr)){dl=1;dr=0;}
  for(int x=l;x<=r;x++)if(x==l||x==r||x<ul||x>ur||x<dl||x>dr)shape_pixel(x,y,c);
 }
}
/* Rounded rectangle at x,y with size w,h and corner radius r (rrect/rrectfill). */
static void rounded_rect(int x,int y,int w,int h,int r,int c,bool fill){
 if(w<0){x+=w+1;w=-w;}if(h<0){y+=h+1;h=-h;}if(!w||!h)return;
 if(w>4096||h>4096)p8_fail("rrect exceeds subset limit");
 int limit=(w<h?w:h)/2;if(r>limit)r=limit;if(r<0)r=0;
 int x1=x+w-1,y1=y+h-1;
 if(!r){if(fill)rectfill(x,y,x1,y1,c);else{line_draw(x,y,x1,y,c);if(y1>y)line_draw(x,y1,x1,y1,c);if(h>2){line_draw(x,y+1,x,y1-1,c);if(x1>x)line_draw(x1,y+1,x1,y1-1,c);}}return;}
 if(fill){
  if(y+r<=y1-r)rectfill(x,y+r,x1,y1-r,c);
  int xx=r,yy=0,err=1-r;
  while(xx>=yy){
   rectfill(x+r-xx,y+r-yy,x1-r+xx,y+r-yy,c);rectfill(x+r-xx,y1-r+yy,x1-r+xx,y1-r+yy,c);
   rectfill(x+r-yy,y+r-xx,x1-r+yy,y+r-xx,c);rectfill(x+r-yy,y1-r+xx,x1-r+yy,y1-r+xx,c);
   yy++;if(err<0)err+=2*yy+1;else{xx--;err+=2*(yy-xx)+1;}
  }
  return;
 }
 if(x+r<=x1-r){line_draw(x+r,y,x1-r,y,c);line_draw(x+r,y1,x1-r,y1,c);}
 if(y+r<=y1-r){line_draw(x,y+r,x,y1-r,c);line_draw(x1,y+r,x1,y1-r,c);}
 int xx=r,yy=0,err=1-r;
 while(xx>=yy){
  shape_pixel(x1-r+xx,y1-r+yy,c);shape_pixel(x+r-xx,y1-r+yy,c);shape_pixel(x1-r+xx,y+r-yy,c);shape_pixel(x+r-xx,y+r-yy,c);
  shape_pixel(x1-r+yy,y1-r+xx,c);shape_pixel(x+r-yy,y1-r+xx,c);shape_pixel(x1-r+yy,y+r-xx,c);shape_pixel(x+r-yy,y+r-xx,c);
  yy++;if(err<0)err+=2*yy+1;else{xx--;err+=2*(yy-xx)+1;}
 }
}
void p8_text(const char *s,int x,int y,int c){int origin=x;for(;*s;s++,x+=4){unsigned ch=(uint8_t)*s;if(ch=='\n'){x=origin-4;y+=6;continue;}
 // Source strings use P8SCII bytes. Common button icons match the physical A/B labels.
 if(ch>=128){if(ch==142)ch='a';else if(ch==151)ch='b';else ch='?';}
 ch&=127;for(int row=0;row<6;row++)for(int col=0;col<4;col++)if(p8_font[ch*6+row]&(1<<col))pixel(x+col,y+row,c);}}
static void sprite(int n,int x,int y,int w,int h,bool fx,bool fy){int sx=(n%16)*8,sy=(n/16)*8;int width=w*8,height=h*8;if(width>1024||height>1024||width<0||height<0)p8_fail("sprite dimensions outside supported range");
 for(int yy=0;yy<height;yy++)for(int xx=0;xx<width;xx++){int c=sample(sx+(fx?width-1-xx:xx),sy+(fy?height-1-yy:yy));if(!transparent[c])pixel(x+xx,y+yy,c);}}
/* Textured line through map space. mx,my,mdx,mdy are fixed-point tile units;
 * hw_ram 0x5f38/0x5f39 give the wrap mask size and 0x5f3a/0x5f3b its offset. */
static void textured_line(int x0,int y0,int x1,int y1,int32_t mx,int32_t my,int32_t mdx,int32_t mdy,int layers){
 if(abs(x1-x0)>4096||abs(y1-y0)>4096)p8_fail("tline too long");
 int dx=abs(x1-x0),dy=abs(y1-y0),steps=dx>dy?dx:dy,sx=x1>x0?1:x1<x0?-1:0,sy=y1>y0?1:y1<y0?-1:0;
 int wrap_w=hw_ram[0x38],wrap_h=hw_ram[0x39],off_x=hw_ram[0x3a],off_y=hw_ram[0x3b];
 int64_t ex=0,ey=0;int x=x0,y=y0;
 for(int i=0;i<=steps;i++){
  int tx=(int)(mx>>16),ty=(int)(my>>16);
  if(wrap_w)tx=off_x+(((tx%wrap_w)+wrap_w)%wrap_w);if(wrap_h)ty=off_y+(((ty%wrap_h)+wrap_h)%wrap_h);
  int tile=mapget(tx,ty);
  if(tile&&(!layers||(flags[tile]&layers))){
   int px=(int)((mx>>13)&7),py=(int)((my>>13)&7);
   int c=sample((tile%16)*8+px,(tile/16)*8+py);
   if(!transparent[c])pixel(x,y,c);
  }
  mx+=mdx;my+=mdy;
  if(steps){ex+=dx;ey+=dy;if(dx>=dy){x+=sx;if(2*ey>=steps){y+=sy;ey-=steps;}}else{y+=sy;if(2*ex>=steps){x+=sx;ex-=steps;}}}
 }
}
static double real(V v){return (double)p8_number(v)/65536.0;}
static V numeric(double n){if(!isfinite(n)||n< -32768.0||n>=32768.0)p8_fail("math result outside fixed point range");return p8_num((int32_t)floor(n*65536.));}
/* Numeric text shared by TONUM, SELECT coercion and SPLIT. Hex/binary
 * construction uses unsigned bits, including the signed 16.16 wrap boundary.
 * Decimal rounding/range behavior remains a documented partial implementation. */
static bool number_space(char c){return c==' '||c=='\t'||c=='\r'||c=='\n'||c=='\v'||c=='\f';}
static int hex_digit(char c){if(c>='0'&&c<='9')return c-'0';if(c>='a'&&c<='f')return c-'a'+10;if(c>='A'&&c<='F')return c-'A'+10;return -1;}
static bool parse_fixed(V value,int32_t *result){
 if(value.tag==P8_NUM){*result=value.n;return true;}
 if(value.tag!=P8_STR)return false;
 const char *start=string(value),*end=start+strlen(start);
 while(start<end&&number_space(*start))start++;
 while(end>start&&number_space(end[-1]))end--;
 const char *at=start;bool negative=false;
 if(at<end&&(*at=='-'||*at=='+')){negative=*at=='-';at++;}
 if(end-at>=2&&at[0]=='0'&&(at[1]=='x'||at[1]=='X'||at[1]=='b'||at[1]=='B')){
  unsigned step=(at[1]=='x'||at[1]=='X')?4:1,base=1u<<step,bits_left=16;
  uint32_t whole=0,fraction=0;bool dot=false,digits=false;at+=2;
  for(;at<end;at++){
   if(*at=='.'&&!dot){dot=true;continue;}
   int digit=hex_digit(*at);if(digit<0||(unsigned)digit>=base)return false;digits=true;
   if(!dot)whole=(whole<<step)|(unsigned)digit;
   else if(bits_left){bits_left-=step;fraction|=(unsigned)digit<<bits_left;}
  }
  if(!digits)return false;
  uint32_t bits=(whole<<16)|fraction;*result=(int32_t)(negative?0u-bits:bits);return true;
 }
 if(at==end||!((*at>='0'&&*at<='9')||*at=='.'))return false;
 char *stop;double number=strtod(start,&stop);
 if(stop!=end||!isfinite(number)||number< -32768.0||number>=32768.0)return false;
 *result=(int32_t)floor(number*65536.0);return true;
}
/* API bitwise operands follow numeric conversion with zero fallback.
 * Operators continue to use p8_number and reject non-number values. */
static int32_t bit_argument(V value){int32_t result;return parse_fixed(value,&result)?result:0;}
static bool parse_raw_integer(V value,int32_t *result){
 if(value.tag!=P8_STR)return false;
 const char *at=string(value),*end=at+strlen(at);
 while(at<end&&number_space(*at))at++;while(end>at&&number_space(end[-1]))end--;
 bool negative=false;if(at<end&&(*at=='-'||*at=='+')){negative=*at=='-';at++;}
 if(at==end)return false;uint32_t bits=0;
 for(;at<end;at++){if(*at<'0'||*at>'9')return false;bits=bits*10u+(unsigned)(*at-'0');}
 *result=(int32_t)(negative?0u-bits:bits);return true;
}
static uint32_t random_next(void){uint32_t x=random_state;x^=x<<13;x^=x>>17;x^=x<<5;return random_state=x?x:1;}
#define HAS(i) ((i)<argc)
#define A(i) (HAS(i)?argv[i]:p8_nil())
#define I(i,def) (HAS(i)?integer(argv[i]):(def))
#define C(i) I(i,draw_color)
#define IS(api) (id==(api))
/* ---- PICO-8 base RAM (0x0000..0x7fff) mapped onto the runtime state ---- */
static int repeat_delay(void){int d=hw_ram[0x5c];return d?d:15;}
static int repeat_interval(void){int i=hw_ram[0x5d];return i?i:4;}
static uint8_t mem_read(int address){
 address&=0xffff;
 if(address<0x2000)return gfx[address];
 if(address<0x3000)return map_data[address-0x2000];
 if(address<0x3100)return flags[address-0x3000];
 if(address<0x4300)return sound_ram[address-0x3100];
 if(address<0x5e00)return user_ram[address-0x4300];
 if(address<0x5f00)return persist_ram[address-0x5e00];
 if(address<0x6000){
  int r=address-0x5f00;
  if(r<0x10)return (uint8_t)(palette[r]|(transparent[r]?0x10:0));
  if(r<0x20)return p8_screen_palette[r-0x10];
  switch(r){
   case 0x20:return (uint8_t)clip_x0;case 0x21:return (uint8_t)clip_y0;case 0x22:return (uint8_t)clip_x1;case 0x23:return (uint8_t)clip_y1;
   case 0x25:return (uint8_t)draw_color;case 0x26:return (uint8_t)cursor_x;case 0x27:return (uint8_t)cursor_y;
   case 0x28:return (uint8_t)camera_x;case 0x29:return (uint8_t)(camera_x>>8);case 0x2a:return (uint8_t)camera_y;case 0x2b:return (uint8_t)(camera_y>>8);
   case 0x31:return (uint8_t)fill_pattern;case 0x32:return (uint8_t)(fill_pattern>>8);case 0x33:return fill_transparent;
  }
  if(r>=0x60&&r<0x70)return secondary_palette[r-0x60];
  return hw_ram[r];
 }
 if(address<0x8000){int at=(address-0x6000)*2;return (uint8_t)((game_frame[at]&15)|((game_frame[at+1]&15)<<4));}
 return 0;/* Upper 32 KB is not mapped on this target; reads return zero. */
}
static void mem_write(int address,uint8_t v){
 address&=0xffff;
 if(address<0x2000){gfx[address]=v;return;}
 if(address<0x3000){map_data[address-0x2000]=v;return;}
 if(address<0x3100){flags[address-0x3000]=v;return;}
 if(address<0x4300){sound_ram[address-0x3100]=v;return;}
 if(address<0x5e00){user_ram[address-0x4300]=v;return;}
 if(address<0x5f00){persist_ram[address-0x5e00]=v;return;}
 if(address<0x6000){
  int r=address-0x5f00;
  if(r<0x10){palette[r]=v&15;transparent[r]=(v&0x10)!=0;return;}
  if(r<0x20){p8_screen_palette[r-0x10]=v&0x8f;return;}
  switch(r){
   case 0x20:clip_x0=screen_bound(v);return;case 0x21:clip_y0=screen_bound(v);return;case 0x22:clip_x1=screen_bound(v);return;case 0x23:clip_y1=screen_bound(v);return;
   case 0x25:draw_color=v;return;case 0x26:cursor_x=v;return;case 0x27:cursor_y=v;return;
   case 0x28:camera_x=(int16_t)((camera_x&0xff00)|v);return;case 0x29:camera_x=(int16_t)((camera_x&0xff)|(v<<8));return;
   case 0x2a:camera_y=(int16_t)((camera_y&0xff00)|v);return;case 0x2b:camera_y=(int16_t)((camera_y&0xff)|(v<<8));return;
   case 0x31:fill_pattern=(uint16_t)((fill_pattern&0xff00)|v);return;case 0x32:fill_pattern=(uint16_t)((fill_pattern&0xff)|(v<<8));return;case 0x33:fill_transparent=v&1;return;
  }
  if(r>=0x60&&r<0x70){secondary_palette[r-0x60]=v;return;}
  hw_ram[r]=v;return;
 }
 if(address<0x8000){int at=(address-0x6000)*2;game_frame[at]=v&15;game_frame[at+1]=v>>4;return;}
 p8_fail("upper memory 0x8000-0xffff is not available on this target");
}
static int address_of(V v){return (int)(((uint32_t)p8_number(v)>>16)&0xffff);}
/* Original cartridge ROM, used by reload(). Sound data is optional for older generated assets. */
static uint8_t rom_read(int address){
 address&=0xffff;
 if(address<0x2000)return p8_cart_gfx[address];
 if(address<0x3000)return p8_cart_map[address-0x2000];
 if(address<0x3100)return p8_cart_flags[address-0x3000];
 if(address<0x4300)return p8_cart_sound?p8_cart_sound[address-0x3100]:0;
 return 0;
}
static void check_range(int address,int length){if(length<0||length>0x8000||address+length>0x10000)p8_fail("memory range outside 64 KB address space");}

/* ---- Stateful silent audio: sequencer state without sample output ---- */
#define AUDIO_RATE 22050
#define NOTE_SAMPLES 183
static struct {int sfx,note,first,last,loop_start,loop_end,elapsed;bool looping,music,release;} voices[4];
static struct {int pattern,patterns_played,ticks,mask,elapsed,length;bool playing;} song;
static const uint8_t *sfx_record(int n){return sound_ram+0x100+(n&63)*68;}
static int sfx_speed(int n){int s=sfx_record(n)[65];return s?s:1;}
static void voice_stop(int ch){voices[ch].sfx=-1;voices[ch].note=-1;}
static void voice_start(int ch,int n,int offset,int length,bool music){
 const uint8_t *r=sfx_record(n);int ls=r[66],le=r[67];
 voices[ch].sfx=n;voices[ch].first=offset<0?0:offset>31?31:offset;voices[ch].note=voices[ch].first;voices[ch].elapsed=0;voices[ch].music=music;voices[ch].release=false;
 /* loop end 0 with a nonzero start makes the start an sfx length. */
 voices[ch].looping=le>ls;voices[ch].loop_start=ls;voices[ch].loop_end=le;
 int last=le==0&&ls>0?ls-1:31;if(length>0&&voices[ch].first+length-1<last)last=voices[ch].first+length-1;voices[ch].last=last;
}
static int pattern_byte(int p,int ch){return sound_ram[(p&63)*4+ch];}
static int pattern_length(int p){
 /* The leftmost enabled non-looping channel sets the pattern length. */
 int fallback=-1;
 for(int ch=0;ch<4;ch++){int b=pattern_byte(p,ch);if(b&0x40)continue;int n=b&63;const uint8_t *r=sfx_record(n);int ls=r[66],le=r[67];
  int notes=le==0&&ls>0?ls:32;int len=notes*sfx_speed(n)*NOTE_SAMPLES;
  if(!(le>ls))return len;if(fallback<0)fallback=len;}
 return fallback>0?fallback:32*NOTE_SAMPLES;
}
static void music_start(int p){
 if(p<0||p>63){song.playing=false;song.pattern=-1;for(int ch=0;ch<4;ch++)if(voices[ch].music)voice_stop(ch);return;}
 song.pattern=p;song.elapsed=0;song.ticks=0;song.playing=true;song.length=pattern_length(p);
 for(int ch=0;ch<4;ch++){int b=pattern_byte(p,ch);if(voices[ch].music)voice_stop(ch);if(!(b&0x40)&&(song.mask&(1<<ch)))voice_start(ch,b&63,0,0,true);}
}
static void music_next(void){
 int p=song.pattern;song.patterns_played++;
 if(pattern_byte(p,2)&0x80){music_start(-1);return;}
 if(pattern_byte(p,1)&0x80){int q=p;while(q>0&&!(pattern_byte(q,0)&0x80))q--;music_start(q);return;}
 if(p+1>63){music_start(-1);return;}
 /* An empty following pattern ends the song. */
 int next=p+1,b=0;for(int ch=0;ch<4;ch++)b|=!(pattern_byte(next,ch)&0x40);
 music_start(b?next:-1);
}
static void audio_advance(int samples){
 for(int ch=0;ch<4;ch++){
  if(voices[ch].sfx<0)continue;
  voices[ch].elapsed+=samples;int duration=sfx_speed(voices[ch].sfx)*NOTE_SAMPLES;
  while(voices[ch].sfx>=0&&voices[ch].elapsed>=duration){
   voices[ch].elapsed-=duration;voices[ch].note++;
   if(voices[ch].looping&&!voices[ch].release&&!voices[ch].music&&voices[ch].note>=voices[ch].loop_end)voices[ch].note=voices[ch].loop_start;
   else if(voices[ch].music&&voices[ch].looping&&voices[ch].note>=voices[ch].loop_end)voices[ch].note=voices[ch].loop_start;
   else if(voices[ch].note>voices[ch].last){voice_stop(ch);}
  }
 }
 if(song.playing){
  song.elapsed+=samples;song.ticks=song.elapsed/NOTE_SAMPLES;
  while(song.playing&&song.elapsed>=song.length){int over=song.elapsed-song.length;music_next();if(song.playing){song.elapsed=over;song.ticks=over/NOTE_SAMPLES;}}
 }
}
static void audio_reset(void){for(int ch=0;ch<4;ch++){voice_stop(ch);voices[ch].music=false;}song.playing=false;song.pattern=-1;song.patterns_played=0;song.ticks=0;song.mask=15;}
static int free_channel(void){for(int ch=0;ch<4;ch++)if(voices[ch].sfx<0)return ch;for(int ch=0;ch<4;ch++)if(!voices[ch].music)return ch;return 0;}

/* ---- Pause-menu items registered by menuitem() ---- */
int p8_menu_count(void){int n=0;for(int i=0;i<5;i++)n+=menu_items[i].label.tag==P8_STR;return n;}
const char *p8_menu_label(int index){if(index<1||index>5||menu_items[index-1].label.tag!=P8_STR)return NULL;return string(menu_items[index-1].label);}
void p8_menu_select(int index,int buttons){
 if(index<1||index>5||menu_items[index-1].callback.tag!=P8_FUNCTION)return;
 budget=200000;V b=p8_int(buttons);p8_call(menu_items[index-1].callback,1,&b);
}

/* ---- Runtime-native closures (iterator values, all(), next, coroutines) ---- */
enum {NATIVE_NEXT=P8_RUNTIME_NATIVE,NATIVE_INEXT,NATIVE_ALL};
static V native_value(int fn,int count,const V *cells){return p8_closure(fn,count,cells);}
/* ---- Coroutines: each one runs native code on its own small stack ---- */
#if defined(__SANITIZE_ADDRESS__)
#include <sanitizer/common_interface_defs.h>
#include <sanitizer/asan_interface.h>
#define P8_ASAN 1
#endif
#ifndef P8_COROUTINES
#if defined(__arm__)
#define P8_COROUTINES 4
#define P8_COROUTINE_STACK 3072
#else
#define P8_COROUTINES 8
#define P8_COROUTINE_STACK 65536
#endif
#endif
enum {CO_FREE,CO_SUSPENDED,CO_RUNNING,CO_NORMAL,CO_DEAD};
static struct {uint8_t state,mark,started;V function;void *sp;int caller;unsigned depth;} coroutines[P8_COROUTINES];
static uint8_t coroutine_stacks[P8_COROUTINES][P8_COROUTINE_STACK] __attribute__((aligned(16)));
static void *main_sp;
static int current_coroutine=-1;
static V transfer[P8_MAX_RESULTS];
static int transfer_count;
void p8_context_switch(void **save,void *next);
#if defined(__x86_64__)
__asm__(".text\n.globl p8_context_switch\n.type p8_context_switch,@function\np8_context_switch:\n"
 " pushq %rbp\n pushq %rbx\n pushq %r12\n pushq %r13\n pushq %r14\n pushq %r15\n"
 " movq %rsp,(%rdi)\n movq %rsi,%rsp\n"
 " popq %r15\n popq %r14\n popq %r13\n popq %r12\n popq %rbx\n popq %rbp\n ret\n"
 ".size p8_context_switch,.-p8_context_switch\n");
#define CONTEXT_WORDS 7
#elif defined(__aarch64__)
__asm__(".text\n.globl p8_context_switch\n.type p8_context_switch,%function\np8_context_switch:\n"
 " sub sp,sp,#176\n stp x19,x20,[sp,#0]\n stp x21,x22,[sp,#16]\n stp x23,x24,[sp,#32]\n stp x25,x26,[sp,#48]\n stp x27,x28,[sp,#64]\n stp x29,x30,[sp,#80]\n"
 " stp d8,d9,[sp,#96]\n stp d10,d11,[sp,#112]\n stp d12,d13,[sp,#128]\n stp d14,d15,[sp,#144]\n"
 " mov x2,sp\n str x2,[x0]\n mov sp,x1\n"
 " ldp x19,x20,[sp,#0]\n ldp x21,x22,[sp,#16]\n ldp x23,x24,[sp,#32]\n ldp x25,x26,[sp,#48]\n ldp x27,x28,[sp,#64]\n ldp x29,x30,[sp,#80]\n"
 " ldp d8,d9,[sp,#96]\n ldp d10,d11,[sp,#112]\n ldp d12,d13,[sp,#128]\n ldp d14,d15,[sp,#144]\n add sp,sp,#176\n ret\n"
 ".size p8_context_switch,.-p8_context_switch\n");
#define CONTEXT_WORDS 0
#elif defined(__arm__) && defined(__thumb__)
/* ARMv6-M: save r4-r11 and lr; high registers move through low registers. */
__asm__(".text\n.syntax unified\n.thumb\n.globl p8_context_switch\n.type p8_context_switch,%function\n.thumb_func\np8_context_switch:\n"
 " push {r4-r7,lr}\n mov r4,r8\n mov r5,r9\n mov r6,r10\n mov r7,r11\n push {r4-r7}\n"
 " mov r2,sp\n str r2,[r0]\n mov sp,r1\n"
 " pop {r4-r7}\n mov r8,r4\n mov r9,r5\n mov r10,r6\n mov r11,r7\n pop {r4-r7,pc}\n"
 ".size p8_context_switch,.-p8_context_switch\n");
#define CONTEXT_WORDS 9
#else
#error "coroutine context switch is not implemented for this architecture"
#endif
static uint8_t *coroutine_top(int i){return coroutine_stacks[i]+P8_COROUTINE_STACK;}
/* Stack bounds for the pico backend's headroom check. */
const void *p8_current_stack_low(void){return current_coroutine<0?NULL:coroutine_stacks[current_coroutine];}
#ifdef P8_ASAN
/* Tell AddressSanitizer about every stack switch so its stack bookkeeping stays valid. */
static const void *main_bottom;static size_t main_size;static int switching_from;
static void after_switch(void *fake){const void *bottom;size_t size;__sanitizer_finish_switch_fiber(fake,&bottom,&size);if(switching_from<0){main_bottom=bottom;main_size=size;}}
static void switch_to(int from,void **save,void *next,int target,bool final){
 void *fake=NULL;switching_from=from;
 __sanitizer_start_switch_fiber(final?NULL:&fake,target<0?main_bottom:coroutine_stacks[target],target<0?main_size:P8_COROUTINE_STACK);
 p8_context_switch(save,next);after_switch(fake);
}
#else
static void switch_to(int from,void **save,void *next,int target,bool final){(void)from;(void)target;(void)final;p8_context_switch(save,next);}
#endif
static void **context_slot(int i){return i<0?&main_sp:&coroutines[i].sp;}
static void coroutine_finish(void);
__attribute__((noreturn,noinline)) static void coroutine_entry(void){
#ifdef P8_ASAN
 after_switch(NULL);
#endif
 int self=current_coroutine;
 V args[P8_MAX_RESULTS];int n=transfer_count;memcpy(args,transfer,sizeof(V)*(size_t)n);
 p8_call(coroutines[self].function,n,args);
 transfer_count=p8_result_count();for(int i=0;i<transfer_count;i++)transfer[i]=p8_result(i);
 coroutine_finish();
 for(;;){}
}
static void coroutine_finish(void){
 int self=current_coroutine,caller=coroutines[self].caller;
 coroutines[self].state=CO_DEAD;coroutines[self].function=p8_nil();
 switch_to(self,&coroutines[self].sp,*context_slot(caller),caller,true);
}
static void prepare_stack(int i){
#ifdef P8_ASAN
 ASAN_UNPOISON_MEMORY_REGION(coroutine_stacks[i],P8_COROUTINE_STACK);
#endif
 uintptr_t top=(uintptr_t)coroutine_top(i)&~(uintptr_t)15;
#if defined(__x86_64__)
 uintptr_t *sp=(uintptr_t*)(top-8*8);memset(sp,0,8*8);sp[6]=(uintptr_t)coroutine_entry;
#elif defined(__aarch64__)
 uintptr_t *sp=(uintptr_t*)(top-176);memset(sp,0,176);sp[11]=(uintptr_t)coroutine_entry;
#else
 uint32_t *sp=(uint32_t*)(top-4*CONTEXT_WORDS);memset(sp,0,4*CONTEXT_WORDS);sp[8]=(uint32_t)(uintptr_t)coroutine_entry;
#endif
 coroutines[i].sp=sp;
}
static int coroutine_id(V v){if(v.tag!=P8_THREAD||(unsigned)v.n>=P8_COROUTINES||coroutines[v.n].state==CO_FREE)p8_fail("coroutine expected");return v.n;}
static int coroutine_slot(void){for(int i=0;i<P8_COROUTINES;i++)if(coroutines[i].state==CO_FREE)return i;return -1;}
static V coroutine_api(int id,int argc,const V *argv){
 if(id==P8_API_cocreate){
  closure_id(A(0));int i=coroutine_slot();
  if(i<0&&can_collect()){collect_now(false);i=coroutine_slot();}
  if(i<0)p8_fail("coroutine pool exhausted");
  coroutines[i].state=CO_SUSPENDED;coroutines[i].mark=0;coroutines[i].started=0;coroutines[i].function=A(0);coroutines[i].depth=0;
  prepare_stack(i);return(V){i,P8_THREAD};
 }
 if(id==P8_API_costatus){
  static const char *const names[]={"dead","suspended","running","normal","dead"};
  if(A(0).tag!=P8_THREAD)p8_fail("coroutine expected");int i=coroutine_id(A(0));return p8_str(names[coroutines[i].state]);
 }
 if(id==P8_API_coresume){
  int i=coroutine_id(A(0));
  if(coroutines[i].state!=CO_SUSPENDED){V out[2]={p8_bool(false),p8_str(coroutines[i].state==CO_DEAD?"cannot resume dead coroutine":"cannot resume non-suspended coroutine")};return p8_return(2,out);}
  transfer_count=argc-1<0?0:argc-1;if(transfer_count>P8_MAX_RESULTS)p8_fail("coresume arguments exceed the result limit");
  for(int n=0;n<transfer_count;n++)transfer[n]=argv[n+1];
  int caller=current_coroutine;if(caller>=0)coroutines[caller].state=CO_NORMAL;
  coroutines[i].caller=caller;coroutines[i].state=CO_RUNNING;coroutines[i].started=1;
  unsigned caller_depth=depth;depth=coroutines[i].depth;current_coroutine=i;
  switch_to(caller,context_slot(caller),coroutines[i].sp,i,false);
  /* Back in the resumer: transfer holds yielded or returned values. */
  depth=caller_depth;current_coroutine=caller;if(caller>=0)coroutines[caller].state=CO_RUNNING;
  V out[P8_MAX_RESULTS];int n=transfer_count+1>P8_MAX_RESULTS?P8_MAX_RESULTS:transfer_count+1;
  out[0]=p8_bool(true);for(int k=1;k<n;k++)out[k]=transfer[k-1];return p8_return(n,out);
 }
 /* yield */
 int self=current_coroutine;if(self<0)p8_fail("yield called outside a coroutine");
 transfer_count=argc>P8_MAX_RESULTS?P8_MAX_RESULTS:argc;for(int n=0;n<transfer_count;n++)transfer[n]=argv[n];
 int caller=coroutines[self].caller;coroutines[self].state=CO_SUSPENDED;coroutines[self].depth=depth;
 switch_to(self,&coroutines[self].sp,*context_slot(caller),caller,false);
 /* Resumed: the resume arguments become yield's results. */
 V out[P8_MAX_RESULTS];int n=transfer_count;memcpy(out,transfer,sizeof(V)*(size_t)n);return p8_return(n,out);
}
static void coroutine_roots(void){
 /* Mark scheduling: a coroutine is reachable through values; running or
  * normal ones are always roots because their stacks are active. */
 for(int i=0;i<P8_COROUTINES;i++)if(coroutines[i].state==CO_RUNNING||coroutines[i].state==CO_NORMAL)shade((V){i,P8_THREAD});
}
static void scan_coroutine(int i){
 shade(coroutines[i].function);
 if(i!=current_coroutine&&coroutines[i].started&&(coroutines[i].state==CO_SUSPENDED||coroutines[i].state==CO_NORMAL))scan_range((const uint8_t*)coroutines[i].sp,coroutine_top(i));
}
static void scan_other_stacks(void){
 if(current_coroutine>=0&&main_sp&&p8_stack_top)scan_range((const uint8_t*)main_sp,(const uint8_t*)p8_stack_top);
}
static const uint8_t *current_stack_top(void){return current_coroutine<0?(const uint8_t*)p8_stack_top:coroutine_top(current_coroutine);}
static void sweep_coroutines(void){for(int i=0;i<P8_COROUTINES;i++){if(coroutines[i].state!=CO_FREE&&!coroutines[i].mark&&coroutines[i].state!=CO_RUNNING&&coroutines[i].state!=CO_NORMAL)coroutines[i].state=CO_FREE;coroutines[i].mark=0;}}
/* -1: not a live coroutine; otherwise the mark bit (optionally setting it). */
static int coroutine_mark(int i,bool set){if((unsigned)i>=P8_COROUTINES||coroutines[i].state==CO_FREE)return -1;if(set)coroutines[i].mark=1;return coroutines[i].mark;}
static void reset_coroutines(void){memset(coroutines,0,sizeof coroutines);current_coroutine=-1;main_sp=NULL;}
V p8_api_id(int id,int argc,const V *argv){
#ifdef P8_API_PROFILE
 if((unsigned)id<P8_API_COUNT)api_profile_hits[id]++;
#endif
 p8_tick();
 if(IS(P8_API_cls)){memset(game_frame,palette[I(0,0)&15],sizeof game_frame);cursor_x=cursor_y=clip_x0=clip_y0=0;clip_x1=clip_y1=128;return p8_nil();}
 if(IS(P8_API_pset)){int c=C(2);if(HAS(2))draw_color=c&255;shape_pixel(I(0,0),I(1,0),c);return p8_nil();}
 if(IS(P8_API_pget)){int x=I(0,0)-camera_x,y=I(1,0)-camera_y;return p8_int((unsigned)x<128&&(unsigned)y<128?game_frame[y*128+x]:0);}
 if(IS(P8_API_rectfill)||IS(P8_API_rect)){int x=I(0,0),y=I(1,0),xx=I(2,0),yy=I(3,0),c=C(4);draw_color=c&255;if(IS(P8_API_rectfill))rectfill(x,y,xx,yy,c);else{line_draw(x,y,xx,y,c);line_draw(xx,y,xx,yy,c);line_draw(xx,yy,x,yy,c);line_draw(x,yy,x,y,c);}return p8_nil();}
 if(IS(P8_API_line)){
  if(argc>5)p8_fail("line accepts at most five arguments");
  if(argc<2){if(argc)draw_color=I(0,6)&15;line_started=false;return p8_nil();}
  int x=I(0,0),y=I(1,0),c=argc<4?C(2):C(4);draw_color=c&255;
  if(argc>=4){line_x=x;line_y=y;x=I(2,0);y=I(3,0);line_started=true;}
  if(line_started)line_draw(line_x,line_y,x,y,c);
  line_x=x;line_y=y;line_started=true;return p8_nil();
 }
 if(IS(P8_API_circfill)||IS(P8_API_circ)){int c=C(3);draw_color=c&255;circle(I(0,0),I(1,0),I(2,4),c,IS(P8_API_circfill));return p8_nil();}
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
 if(IS(P8_API_color)){int old=draw_color;draw_color=I(0,6)&255;return p8_int(old);}
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
 if(IS(P8_API_pal)){
  if(argc>3)p8_fail("pal accepts at most three arguments");
  bool table=A(0).tag==P8_TABLE;
  int which=table?I(1,0):argc==1?I(0,0):I(2,0);
  if(which<0||which>2)p8_fail("pal palette selector must be 0, 1 or 2");
  if(!argc||(!table&&argc==1)){
   /* pal() resets every palette; pal(p) resets one. */
   if(!argc||which==0)for(int i=0;i<16;i++){palette[i]=i;transparent[i]=(i==0);}
   if(!argc||which==1)for(int i=0;i<16;i++)p8_screen_palette[i]=i;
   if(!argc||which==2)for(int i=0;i<16;i++)secondary_palette[i]=i;
   return p8_nil();
  }
  if(table){
   if(argc>2)p8_fail("table pal accepts at most two arguments");
   int tid=table_id(A(0));
   /* Keep this overload bounded to the documented palette/list indices. */
   for(int at=tables[tid].head;at;at=entries[at-1].next){
    V key=ENTRY_KEY(at-1),value=ENTRY_VALUE(at-1);
    if(key.tag!=P8_NUM||(key.n&65535)||key.n<0||key.n>16*65536)
     p8_fail("pal table keys must be integers from 0 to 16");
    if(value.tag!=P8_NUM)p8_fail("pal table values must be numbers");
   }
   if(raw_get(A(0),p8_int(0)).tag!=P8_NIL&&raw_get(A(0),p8_int(16)).tag!=P8_NIL)
    p8_fail("pal table cannot contain both color zero indices");
   for(int i=0;i<=16;i++){
    V value=raw_get(A(0),p8_int(i));
    if(value.tag==P8_NIL)continue;
    int c=integer(value);
    if(which==0)palette[i&15]=c&15;else if(which==1)p8_screen_palette[i&15]=c&0x8f;else secondary_palette[i&15]=c&255;
   }
   return p8_nil();
  }
  int color=I(0,0)&15,old,to=I(1,0);
  if(which==0){old=palette[color]|(transparent[color]<<4);palette[color]=to&15;}
  else if(which==1){old=p8_screen_palette[color];p8_screen_palette[color]=to&0x8f;}
  else{old=secondary_palette[color];secondary_palette[color]=to&255;}
  return p8_int(old);
 }
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
 if(IS(P8_API_fget)){int tile=I(0,0),bit=I(1,0),f=(unsigned)tile<256?flags[tile]:0;return argc>1?p8_bool((unsigned)bit<8&&((f>>bit)&1)):p8_int(f);}
 if(IS(P8_API_fset)){int i=I(0,0)&255;if(argc==2)flags[i]=I(1,0);else if(p8_truth(A(2)))flags[i]|=1<<(I(1,0)&7);else flags[i]&=~(1<<(I(1,0)&7));return p8_nil();}
 if(IS(P8_API_map)){int mx=I(0,0),my=I(1,0),dx=I(2,0),dy=I(3,0),w=I(4,128),h=I(5,32),mask=I(6,0);if(w>128||h>64)p8_fail("map dimensions outside supported range");for(int y=0;y<h;y++)for(int x=0;x<w;x++){int tile=mapget(mx+x,my+y);if(tile&&(!mask||(flags[tile]&mask)))sprite(tile,dx+x*8,dy+y*8,1,1,false,false);}return p8_nil();}
 if(IS(P8_API_btn)||IS(P8_API_btnp)){int player=I(1,0);if(player<0||player>7)p8_fail("player index must be 0 to 7");unsigned bits=IS(P8_API_btn)?game_buttons:pressed;
  /* Only player one has physical buttons; other documented players read as idle. */
  if(!argc)return p8_int((int)bits);if(player)return p8_bool(false);int b=I(0,0);return p8_bool((unsigned)b<6&&(bits&(1u<<b)));}
 if(IS(P8_API_rawget)){if(argc!=2)p8_fail("rawget requires two arguments");return raw_get(A(0),A(1));}
 if(IS(P8_API_rawset)){if(argc!=3)p8_fail("rawset requires three arguments");raw_set(A(0),A(1),A(2));return A(0);}
 if(IS(P8_API_rawequal)){if(argc!=2)p8_fail("rawequal requires two arguments");return p8_bool(equal(A(0),A(1)));}
 if(IS(P8_API_rawlen)){if(argc!=1)p8_fail("rawlen requires one argument");return p8_int(raw_length(A(0)));}
 if(IS(P8_API_select)){
  if(argc<1||argc>P8_MAX_RESULTS)p8_fail("select requires 1 to 16 arguments");
  if(A(0).tag==P8_STR&&string(A(0))[0]=='#'){V n=p8_int(argc-1);return p8_return(1,&n);}
  int32_t selector;if(!parse_fixed(A(0),&selector))p8_fail("number expected");
  int index=selector/65536;
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
   if(convert&&len){int32_t number;if(parse_fixed(v,&number))v=p8_num(number);}
   p8_set(out,p8_int(++n),v);
   if(chunk){at=end;if(!*at)break;}else{if(!*end)break;at=end+seplen;}
  }while(true);
  return out;
 }
 if(IS(P8_API_band)||IS(P8_API_bor)||IS(P8_API_bxor)){int32_t value=bit_argument(A(0));for(int i=1;i<argc;i++){int32_t other=bit_argument(A(i));if(IS(P8_API_band))value&=other;else if(IS(P8_API_bor))value|=other;else value^=other;}return p8_num(value);}
 if(IS(P8_API_bnot))return p8_num(~bit_argument(A(0)));
 if(IS(P8_API_shl)||IS(P8_API_shr)||IS(P8_API_lshr)||IS(P8_API_rotl)||IS(P8_API_rotr)){
  int amount=bit_argument(A(1))>>16;uint32_t bits=(uint32_t)bit_argument(A(0));bool left=IS(P8_API_shl)||IS(P8_API_rotl);
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
 if(IS(P8_API_rnd)){uint32_t r=random_next();if(argc&&A(0).tag==P8_TABLE){int n=raw_length(A(0));return n?raw_get(A(0),p8_int((int)(r%(unsigned)n)+1)):p8_nil();}int32_t limit=HAS(0)?p8_number(A(0)):65536;if(limit<0)p8_fail("negative rnd range unsupported");return p8_num((int32_t)(((uint64_t)r*(uint32_t)limit)>>32));}
 if(IS(P8_API_time)||IS(P8_API_t))return p8_num((int32_t)(((uint64_t)p8_frames*65536)/p8_target_hz));
 if(IS(P8_API_tostr)){
  if(!argc)return p8_str("");
  int flags=HAS(1)&&A(1).tag==P8_BOOL?(p8_truth(A(1))?1:0):I(1,0);
  if(flags&~3)p8_fail("unsupported tostr flags");
  if(A(0).tag==P8_NUM&&flags){
   char text[32];uint32_t bits=(uint32_t)A(0).n;
   if(flags==1)snprintf(text,sizeof text,"0x%04x.%04x",(unsigned)(bits>>16),(unsigned)(bits&65535));
   else if(flags==3)snprintf(text,sizeof text,"0x%08x",(unsigned)bits);
   else snprintf(text,sizeof text,"%ld",(long)A(0).n);
   return p8_str(text);
  }
  if((flags&1)&&(A(0).tag==P8_TABLE||A(0).tag==P8_FUNCTION))p8_fail("raw object-address formatting unsupported");
  if(A(0).tag==P8_STR)return A(0);
  if(A(0).tag==P8_NIL)return p8_str("[nil]");
  char text[128];return p8_str(p8_describe(A(0),text,sizeof text));
 }
 if(IS(P8_API_tonum)){
  int flags=I(1,0);if(flags&~7)p8_fail("unsupported tonum flags");
  int32_t number=0;bool valid=false;
  if(A(0).tag==P8_NUM){number=A(0).n;valid=true;}
  else if((flags&1)&&A(0).tag==P8_STR){
   uint32_t bits=0;for(const char *at=string(A(0));*at;at++){int digit=hex_digit(*at);bits=(bits<<4)|(digit<0?0u:(unsigned)digit);}
   number=(int32_t)((flags&2)?bits:bits<<16);valid=true;
  }else valid=(flags&2)?parse_raw_integer(A(0),&number):parse_fixed(A(0),&number);
  if(!valid&&!(flags&4))return p8_return(0,NULL);
  V result=p8_num(number);return p8_return(1,&result);
 }
 if(IS(P8_API_type)){static const char *types[]={"nil","number","boolean","string","table","function"};return p8_str(types[A(0).tag<=P8_FUNCTION?A(0).tag:0]);}
 if(IS(P8_API_ord)){
  int count=I(2,1);if(count<1||count>P8_MAX_RESULTS)p8_fail("ord count must be 1 to 32 in this subset");
  int index=I(1,1);V results[P8_MAX_RESULTS];
  if(A(0).tag!=P8_STR){if(count!=1)p8_fail("ord multiple results require a string");results[0]=p8_nil();return p8_return(1,results);}
  const char *text=string(A(0));int size=string_length(A(0));
  if(index<1||index>size){if(count!=1)p8_fail("ord result range outside string unsupported");results[0]=p8_nil();return p8_return(1,results);}
  if(count>size-index+1)p8_fail("ord result range outside string unsupported");
  for(int n=0;n<count;n++)results[n]=p8_int((uint8_t)text[index-1+n]);
  return p8_return(count,results);
 }
 if(IS(P8_API_chr)){if(argc>=SL)p8_fail("chr result exceeds string limit");for(int i=0;i<argc;i++)string_scratch[i]=(char)(I(i,0)&255);return string_bytes(string_scratch,(size_t)argc);}
 if(IS(P8_API_sub)){const char *s=string(A(0));int len=string_length(A(0)),i=I(1,1),j=HAS(2)&&A(2).tag!=P8_NUM?i:I(2,len);if(i<0)i+=len+1;if(j<0)j+=len+1;if(i<1)i=1;if(j>len)j=len;int n=j>=i?j-i+1:0;if(i>len)n=0;return string_bytes(n?s+i-1:s,(size_t)n);}
 if(IS(P8_API_sfx)){
  /* Silent but stateful: channel/note state advances and is visible to stat(). */
  int n=I(0,0),ch=I(1,-1),offset=I(2,0),len=I(3,0);
  if(n==-1){if(ch<0){for(int c=0;c<4;c++)if(!voices[c].music)voice_stop(c);}else if(ch<4)voice_stop(ch);return p8_nil();}
  if(n==-2){if(ch<0){for(int c=0;c<4;c++)voices[c].release=true;}else if(ch<4)voices[ch].release=true;return p8_nil();}
  if(n<0||n>63)return p8_nil();
  if(ch==-2){for(int c=0;c<4;c++)if(voices[c].sfx==n)voice_stop(c);return p8_nil();}
  if(ch<0||ch>3){ch=-1;for(int c=0;c<4;c++)if(voices[c].sfx==n&&!voices[c].music){ch=c;break;}if(ch<0)ch=free_channel();}
  voice_start(ch,n,offset,len,false);return p8_nil();
 }
 if(IS(P8_API_music)){
  int n=I(0,0),mask=I(2,0);song.mask=mask&15?mask&15:15;song.patterns_played=0;music_start(n);return p8_nil();
 }
 if(IS(P8_API_printh)){/* File output is a host feature; the text still goes to the serial log. */char description[128];const char *s=A(0).tag==P8_STR?string(A(0)):p8_describe(A(0),description,sizeof description);puts(s);return p8_nil();}
 if(IS(P8_API_stat)){
  int n=I(0,-1);
  switch(n){
   case 0:return p8_num((int32_t)(((uint64_t)p8_heap_entries*2048*65536)/NE));/* approximate Lua memory use, KB */
   case 1:case 2:/* Native budget ratio, not an emulated PICO-8 CPU measurement. */return p8_num((int32_t)(((uint64_t)(200000-budget)*65536)/200000));
   case 4:case 6:return p8_str("");
   case 5:return p8_int(41);
   case 7:case 8:case 9:return p8_int((int)p8_target_hz);
   case 11:return p8_int(1);
   case 28:case 30:case 39:return p8_bool(false);
   case 31:return p8_str("");
   case 32:case 33:case 34:case 36:case 38:return p8_int(0);
   case 100:case 101:return p8_nil();
   case 102:return p8_int(0);
   case 110:return p8_bool(false);
  }
  if(n>=16&&n<=19)return p8_int(voices[n-16].sfx);
  if(n>=46&&n<=49)return p8_int(voices[n-46].sfx);
  if(n>=20&&n<=23)return p8_int(voices[n-20].sfx<0?-1:voices[n-20].note);
  if(n>=50&&n<=53)return p8_int(voices[n-50].sfx<0?-1:voices[n-50].note);
  if(n==24||n==54)return p8_int(song.playing?song.pattern:-1);
  if(n==25||n==55)return p8_int(song.patterns_played);
  if(n==26||n==56)return p8_int(song.playing?song.ticks:0);
  if(n==57)return p8_bool(song.playing);
  /* No real-time clock: dates report the PICO-8 epoch plus elapsed frames. */
  if((n>=80&&n<=85)||(n>=90&&n<=95)){uint32_t sec=p8_frames/p8_target_hz;int k=n%10;int v=k==0?2000:k==1?1:k==2?1:k==3?(int)(sec/3600%24):k==4?(int)(sec/60%60):(int)(sec%60);return p8_int(v);}
  p8_fail("stat index unsupported");return p8_nil();
 }
 if(IS(P8_API_assert)){if(argc<1||argc>P8_MAX_RESULTS)p8_fail("assert requires 1 to 16 arguments");if(!p8_truth(A(0)))p8_fail(argc>1&&A(1).tag==P8_STR?string(A(1)):"cartridge assertion failed");return p8_return(argc,argv);}
 /* ---- Memory ---- */
 if(IS(P8_API_peek)||IS(P8_API_peek2)||IS(P8_API_peek4)){
  int address=HAS(0)?address_of(A(0)):0,count=I(1,1),size=IS(P8_API_peek)?1:IS(P8_API_peek2)?2:4;
  if(count<1)return p8_return(0,NULL);if(count>P8_MAX_RESULTS)p8_fail("peek count exceeds the result limit");
  V out[P8_MAX_RESULTS];
  for(int n=0;n<count;n++){int at=address+n*size;
   if(size==1)out[n]=p8_int(mem_read(at));
   else if(size==2)out[n]=p8_int((int16_t)(mem_read(at)|(mem_read(at+1)<<8)));
   else out[n]=p8_num((int32_t)((uint32_t)mem_read(at)|((uint32_t)mem_read(at+1)<<8)|((uint32_t)mem_read(at+2)<<16)|((uint32_t)mem_read(at+3)<<24)));}
  return p8_return(count,out);
 }
 if(IS(P8_API_poke)||IS(P8_API_poke2)||IS(P8_API_poke4)){
  int address=HAS(0)?address_of(A(0)):0,size=IS(P8_API_poke)?1:IS(P8_API_poke2)?2:4;
  for(int n=1;n<argc||n==1;n++){
   uint32_t v=HAS(n)&&A(n).tag==P8_NUM?(uint32_t)A(n).n:0;if(size<4)v>>=16;
   int at=address+(n-1)*size;for(int b=0;b<size;b++)mem_write(at+b,(uint8_t)(v>>(8*b)));
   if(argc<2)break;
  }
  return p8_nil();
 }
 if(IS(P8_API_memcpy)){
  int dst=address_of(A(0)),src=address_of(A(1)),len=I(2,0);if(len<=0)return p8_nil();check_range(dst,len);check_range(src,len);
  /* Overlapping copies behave like memmove. */
  if(dst<src)for(int i=0;i<len;i++)mem_write(dst+i,mem_read(src+i));else for(int i=len-1;i>=0;i--)mem_write(dst+i,mem_read(src+i));
  return p8_nil();
 }
 if(IS(P8_API_memset)){int dst=address_of(A(0)),v=I(1,0)&255,len=I(2,0);if(len<=0)return p8_nil();check_range(dst,len);for(int i=0;i<len;i++)mem_write(dst+i,(uint8_t)v);return p8_nil();}
 if(IS(P8_API_reload)){
  if(HAS(3)&&A(3).tag!=P8_NIL)p8_fail("reload from another cartridge file is unsupported");
  int dst=HAS(0)?address_of(A(0)):0,src=HAS(1)?address_of(A(1)):0,len=I(2,0x4300);if(len<=0)return p8_nil();check_range(dst,len);check_range(src,len);
  for(int i=0;i<len;i++)mem_write(dst+i,rom_read(src+i));return p8_nil();
 }
 /* ---- Persistence: 64 numbers at 0x5e00. Values last until power-off. ---- */
 if(IS(P8_API_cartdata)){if(cart_data_open)p8_fail("cartdata() may only be called once");if(A(0).tag!=P8_STR)p8_fail("cartdata id must be a string");cart_data_open=true;return p8_bool(false);}
 if(IS(P8_API_dget)){int i=I(0,0);if(i<0||i>63)return p8_int(0);int at=0x5e00+i*4;return p8_num((int32_t)((uint32_t)mem_read(at)|((uint32_t)mem_read(at+1)<<8)|((uint32_t)mem_read(at+2)<<16)|((uint32_t)mem_read(at+3)<<24)));}
 if(IS(P8_API_dset)){int i=I(0,0);if(i<0||i>63)return p8_nil();uint32_t v=A(1).tag==P8_NUM?(uint32_t)A(1).n:0;for(int b=0;b<4;b++)mem_write(0x5e00+i*4+b,(uint8_t)(v>>(8*b)));return p8_nil();}
 /* ---- Menu, frame and program control ---- */
 if(IS(P8_API_menuitem)){
  int index=I(0,0);if(index<1||index>5)return p8_nil();
  if(argc<2||A(1).tag==P8_NIL){menu_items[index-1].label=p8_nil();menu_items[index-1].callback=p8_nil();return p8_nil();}
  if(A(1).tag!=P8_STR)p8_fail("menuitem label must be a string");
  menu_items[index-1].label=A(1);menu_items[index-1].callback=HAS(2)?A(2):p8_nil();return p8_nil();
 }
 if(IS(P8_API_flip)){p8_present();return p8_nil();}
 if(IS(P8_API_run)){longjmp(p8_guard,P8_RESTART);}
 if(IS(P8_API_reset)){
  for(int i=0;i<16;i++){palette[i]=i;transparent[i]=(i==0);p8_screen_palette[i]=i;secondary_palette[i]=i;}
  camera_x=camera_y=cursor_x=cursor_y=clip_x0=clip_y0=0;clip_x1=clip_y1=128;draw_color=6;fill_pattern=0;fill_transparent=false;line_started=false;
  return p8_nil();
 }
 if(IS(P8_API_stop)){char text[96];snprintf(text,sizeof text,"stop(): %s",HAS(0)&&A(0).tag==P8_STR?string(A(0)):"program stopped");p8_fail(text);}
 if(IS(P8_API_extcmd)){
  const char *cmd=A(0).tag==P8_STR?string(A(0)):"";
  if(!strcmp(cmd,"reset"))longjmp(p8_guard,P8_RESTART);
  if(!strcmp(cmd,"pause")){p8_paused_by_cart=true;return p8_nil();}
  if(!strcmp(cmd,"shutdown")||!strcmp(cmd,"go_back"))p8_fail("extcmd: cartridge requested shutdown");
  /* Screenshot, recording, label and audio-capture commands need the PICO-8 host. */
  return p8_nil();
 }
 /* ---- Additional shapes ---- */
 if(IS(P8_API_oval)||IS(P8_API_ovalfill)){int c=C(4);draw_color=c&255;oval(I(0,0),I(1,0),I(2,0),I(3,0),c,IS(P8_API_ovalfill));return p8_nil();}
 if(IS(P8_API_rrect)||IS(P8_API_rrectfill)){int c=C(5);draw_color=c&255;rounded_rect(I(0,0),I(1,0),I(2,0),I(3,0),I(4,0),c,IS(P8_API_rrectfill));return p8_nil();}
 if(IS(P8_API_fillp)){
  int old=fill_pattern|(fill_transparent?0:0);int32_t old_value=(int32_t)((uint32_t)fill_pattern<<16)|(fill_transparent?0x8000:0);(void)old;
  int32_t p=HAS(0)&&A(0).tag==P8_NUM?A(0).n:0;fill_pattern=(uint16_t)((uint32_t)p>>16);fill_transparent=(p&0x8000)!=0;
  return p8_num(old_value);
 }
 if(IS(P8_API_tline)){
  if(argc<6)p8_fail("unsupported tline() overload");
  int32_t mdx=HAS(6)?p8_number(A(6)):0x2000,mdy=HAS(7)?p8_number(A(7)):0;
  textured_line(I(0,0),I(1,0),I(2,0),I(3,0),p8_number(A(4)),p8_number(A(5)),mdx,mdy,I(8,0));return p8_nil();
 }
 /* ---- Metatables ---- */
 if(IS(P8_API_setmetatable)){
  int id=table_id(A(0));if(A(1).tag==P8_NIL)tables[id].meta=0;else tables[id].meta=(uint16_t)(table_id(A(1))+1);return A(0);
 }
 if(IS(P8_API_getmetatable)){
  if(A(0).tag!=P8_TABLE)return p8_nil();V m=metatable(A(0));if(m.tag==P8_NIL)return m;
  V protected_value=raw_get(m,p8_str("__metatable"));return protected_value.tag!=P8_NIL?protected_value:m;
 }
 /* ---- Iterators and table helpers as first-class functions ---- */
 if(IS(P8_API_next)){
  V k,v;if(!table_next(A(0),A(1),&k,&v)){V nil=p8_nil();return p8_return(1,&nil);}V out[2]={k,v};return p8_return(2,out);
 }
 if(IS(P8_API_pairs)){
  V t=A(0);V h=t.tag==P8_TABLE?metamethod(t,"__pairs"):p8_nil();
  if(h.tag!=P8_NIL){V r=p8_call(h,1,&t);V out[3]={r,p8_result(1),p8_result(2)};return p8_return(3,out);}
  V out[3]={native_value(NATIVE_NEXT,0,NULL),t,p8_nil()};table_id(t);return p8_return(3,out);
 }
 if(IS(P8_API_ipairs)){V out[3]={native_value(NATIVE_INEXT,0,NULL),A(0),p8_int(0)};table_id(A(0));return p8_return(3,out);}
 if(IS(P8_API_all)){
  /* all(t) returns a closure over the table and its position. */
  table_id(A(0));V cells[3]={p8_cell(A(0)),p8_cell(p8_int(0)),p8_cell(p8_nil())};return native_value(NATIVE_ALL,3,cells);
 }
 if(IS(P8_API_unpack)){
  int first=I(1,1),last=HAS(2)&&A(2).tag!=P8_NIL?I(2,0):raw_length(A(0));
  if(last<first)return p8_return(0,NULL);if(last-first+1>P8_MAX_RESULTS)p8_fail("unpack exceeds the result limit");
  V out[P8_MAX_RESULTS];for(int i=first;i<=last;i++)out[i-first]=p8_get(A(0),p8_int(i));return p8_return(last-first+1,out);
 }
 if(IS(P8_API_pack)){V t=p8_table();for(int i=0;i<argc;i++)p8_set(t,p8_int(i+1),argv[i]);p8_set(t,p8_str("n"),p8_int(argc));return t;}
 /* ---- Coroutines ---- */
 if(IS(P8_API_cocreate)||IS(P8_API_coresume)||IS(P8_API_costatus)||IS(P8_API_yield))return coroutine_api(id,argc,argv);
 p8_fail("unimplemented API");return p8_nil();
}
static V runtime_native(int fn,V self,int argc,const V *argv){
 if(fn==NATIVE_NEXT){V k,v;if(!table_next(A(0),A(1),&k,&v)){V nil=p8_nil();return p8_return(1,&nil);}V out[2]={k,v};return p8_return(2,out);}
 if(fn==NATIVE_INEXT){int i=I(1,0)+1;V v=p8_get(A(0),p8_int(i));if(v.tag==P8_NIL){V nil=p8_nil();return p8_return(1,&nil);}V out[2]={p8_int(i),v};return p8_return(2,out);}
 if(fn==NATIVE_ALL){
  V t=p8_cell_get(p8_capture(self,0));int i=integer(p8_cell_get(p8_capture(self,1)));V last=p8_cell_get(p8_capture(self,2));
  /* As in the direct all() loop, revisit a slot whose value changed (del shift). */
  if(!(i>0&&!equal(raw_get(t,p8_int(i)),last)))i++;
  V v=raw_get(t,p8_int(i));p8_cell_set(p8_capture(self,1),p8_int(v.tag==P8_NIL?i-1:i));p8_cell_set(p8_capture(self,2),v);
  return p8_return(1,&v);
 }
 p8_fail("invalid runtime native function");return p8_nil();
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
  [P8_API_peek]="peek",
  [P8_API_poke]="poke",
  [P8_API_peek2]="peek2",
  [P8_API_poke2]="poke2",
  [P8_API_peek4]="peek4",
  [P8_API_poke4]="poke4",
  [P8_API_memcpy]="memcpy",
  [P8_API_memset]="memset",
  [P8_API_reload]="reload",
  [P8_API_cstore]="cstore",
  [P8_API_cartdata]="cartdata",
  [P8_API_dget]="dget",
  [P8_API_dset]="dset",
  [P8_API_menuitem]="menuitem",
  [P8_API_flip]="flip",
  [P8_API_run]="run",
  [P8_API_reset]="reset",
  [P8_API_stop]="stop",
  [P8_API_extcmd]="extcmd",
  [P8_API_oval]="oval",
  [P8_API_ovalfill]="ovalfill",
  [P8_API_rrect]="rrect",
  [P8_API_rrectfill]="rrectfill",
  [P8_API_fillp]="fillp",
  [P8_API_tline]="tline",
  [P8_API_setmetatable]="setmetatable",
  [P8_API_getmetatable]="getmetatable",
  [P8_API_all]="all",
  [P8_API_pairs]="pairs",
  [P8_API_ipairs]="ipairs",
  [P8_API_next]="next",
  [P8_API_unpack]="unpack",
  [P8_API_pack]="pack",
  [P8_API_cocreate]="cocreate",
  [P8_API_coresume]="coresume",
  [P8_API_costatus]="costatus",
  [P8_API_yield]="yield",
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
void p8_reset(void){
 memset(tables,0,sizeof tables);memset(entries,0,sizeof entries);memset(strings,0,sizeof strings);memset(cells,0,sizeof cells);memset(closures,0,sizeof closures);
 for(int i=0;i<NU;i++){captures[i].cell=NO_LINK;captures[i].next=0;}
 memcpy(gfx,p8_cart_gfx,sizeof gfx);memcpy(map_data,p8_cart_map,sizeof map_data);memcpy(flags,p8_cart_flags,sizeof flags);
 if(p8_cart_sound)memcpy(sound_ram,p8_cart_sound,sizeof sound_ram);else memset(sound_ram,0,sizeof sound_ram);
 memset(user_ram,0,sizeof user_ram);memset(persist_ram,0,sizeof persist_ram);memset(hw_ram,0,sizeof hw_ram);memset(game_frame,0,sizeof game_frame);
 for(int i=0;i<16;i++){palette[i]=i;transparent[i]=(i==0);p8_screen_palette[i]=i;secondary_palette[i]=i;}
 memset(held_frames,0,sizeof held_frames);camera_x=camera_y=clip_x0=clip_y0=cursor_x=cursor_y=0;clip_x1=clip_y1=128;draw_color=6;
 game_buttons=previous=pressed=0;p8_frames=p8_operations=p8_heap_entries=p8_heap_strings=0;depth=0;budget=200000;random_state=1;p8_error[0]=0;result_count=0;
 line_x=line_y=0;line_started=false;fill_pattern=0;fill_transparent=false;cart_data_open=false;p8_paused_by_cart=false;
 for(int i=0;i<5;i++){menu_items[i].label=p8_nil();menu_items[i].callback=p8_nil();}
 audio_reset();reset_coroutines();collecting=false;p8_program_clear();}
static void update_input(uint8_t held){
 game_buttons=held;pressed=held&~previous;int delay=repeat_delay(),interval=repeat_interval();
 for(int i=0;i<6;i++){if(held&(1u<<i)){if(held_frames[i]<65535)held_frames[i]++;
  /* 0x5f5c/0x5f5d configure repeat; a delay of 255 disables it. */
  if(delay!=255&&held_frames[i]>=delay+1&&((held_frames[i]-delay-1)%interval)==0)pressed|=1u<<i;}else held_frames[i]=0;}
 previous=held;}
void p8_begin_frame(uint8_t held){budget=200000;depth=0;result_count=0;update_input(held);}
void p8_flip_frame(uint8_t held){budget=200000;update_input(held);}
void p8_end_frame(void){audio_advance(AUDIO_RATE/(int)p8_target_hz);p8_collect();p8_frames++;}
/* flip() inside generated code: collect without compaction, then let the backend present. */
__attribute__((weak)) void p8_present(void){p8_fail("flip() requires a frame backend");}
void p8_flip_collect(void){audio_advance(AUDIO_RATE/(int)p8_target_hz);if(can_collect())collect_now(false);p8_frames++;}
