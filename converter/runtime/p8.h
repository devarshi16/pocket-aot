#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
#include <setjmp.h>

typedef struct { int32_t n; uint8_t tag; } V;
enum { P8_NIL, P8_NUM, P8_BOOL, P8_STR, P8_TABLE, P8_FUNCTION, P8_CELL, P8_THREAD };
#define P8_MAX_STRING 32767
#define P8_MAX_RESULTS 32
/* The screen in PICO-8's own layout (0x6000): two pixels per byte, left pixel
 * in the low nibble. Values are draw-palette colours 0-15. */
extern uint8_t game_frame[128*64], game_buttons;
static inline int p8_frame_get(int x,int y){uint8_t b=game_frame[y*64+(x>>1)];return x&1?b>>4:b&15;}
static inline void p8_frame_set(int x,int y,int c){uint8_t *b=&game_frame[y*64+(x>>1)];*b=x&1?(uint8_t)((*b&15)|((c&15)<<4)):(uint8_t)((*b&0xf0)|(c&15));}
extern const uint8_t pico_palette[16][3];
extern const char *p8_title;
extern const unsigned p8_target_hz;
extern jmp_buf p8_guard;
extern char p8_error[128];
extern uint32_t p8_frames,p8_operations,p8_heap_entries,p8_heap_strings;

static inline V p8_nil(void){return (V){0,P8_NIL};}
static inline V p8_num(int32_t n){return (V){n,P8_NUM};}
static inline V p8_bool(bool b){return (V){b,P8_BOOL};}
static inline V p8_int(int n){return p8_num((int32_t)((uint32_t)n<<16));}
bool p8_truth(V v);
int32_t p8_number(V v);
V p8_str(const char *s);
V p8_strn(const char *s,size_t length);
V p8_table(void);
V p8_get(V t,V key);
void p8_set(V t,V key,V value);
/* Native C closures. Captured values are shared lexical cells, never bytecode. */
V p8_cell(V initial);
V p8_cell_get(V cell);
void p8_cell_set(V cell,V value);
V p8_closure(int fn,int count,const V *cells);
V p8_capture(V self,int index);
V p8_call(V function,int argc,const V *argv);
V p8_return(int count,const V *values);
int p8_result_count(void);
V p8_result(int index);
V p8_dispatch(int fn,V self,int argc,const V *argv);
bool p8_iter_next(const char *kind,V table,int *cursor,V *last,V *key,V *value);
V p8_unary(const char *op,V a);
V p8_binary(const char *op,V a,V b);
/* Stable names shared by generated C and the native API dispatcher. */
enum P8ApiId {
 P8_API_cls,
 P8_API_pset,
 P8_API_pget,
 P8_API_rectfill,
 P8_API_rect,
 P8_API_line,
 P8_API_circfill,
 P8_API_circ,
 P8_API_print,
 P8_API_color,
 P8_API_cursor,
 P8_API_camera,
 P8_API_clip,
 P8_API_pal,
 P8_API_palt,
 P8_API_spr,
 P8_API_sspr,
 P8_API_sget,
 P8_API_sset,
 P8_API_mget,
 P8_API_mset,
 P8_API_fget,
 P8_API_fset,
 P8_API_map,
 P8_API_btn,
 P8_API_btnp,
 P8_API_add,
 P8_API_del,
 P8_API_deli,
 P8_API_count,
 P8_API_foreach,
 P8_API_split,
 P8_API_band,
 P8_API_bor,
 P8_API_bxor,
 P8_API_bnot,
 P8_API_shl,
 P8_API_shr,
 P8_API_lshr,
 P8_API_rotl,
 P8_API_rotr,
 P8_API_flr,
 P8_API_ceil,
 P8_API_abs,
 P8_API_min,
 P8_API_max,
 P8_API_mid,
 P8_API_sgn,
 P8_API_sqrt,
 P8_API_sin,
 P8_API_cos,
 P8_API_atan2,
 P8_API_srand,
 P8_API_rnd,
 P8_API_time,
 P8_API_t,
 P8_API_tostr,
 P8_API_tonum,
 P8_API_type,
 P8_API_ord,
 P8_API_chr,
 P8_API_sub,
 P8_API_sfx,
 P8_API_music,
 P8_API_printh,
 P8_API_stat,
 P8_API_assert,
 P8_API_rawget,
 P8_API_rawset,
 P8_API_rawequal,
 P8_API_rawlen,
 P8_API_select,
 P8_API_peek,
 P8_API_poke,
 P8_API_peek2,
 P8_API_poke2,
 P8_API_peek4,
 P8_API_poke4,
 P8_API_memcpy,
 P8_API_memset,
 P8_API_reload,
 P8_API_cstore,
 P8_API_cartdata,
 P8_API_dget,
 P8_API_dset,
 P8_API_menuitem,
 P8_API_flip,
 P8_API_run,
 P8_API_reset,
 P8_API_stop,
 P8_API_extcmd,
 P8_API_oval,
 P8_API_ovalfill,
 P8_API_rrect,
 P8_API_rrectfill,
 P8_API_fillp,
 P8_API_tline,
 P8_API_setmetatable,
 P8_API_getmetatable,
 P8_API_all,
 P8_API_pairs,
 P8_API_ipairs,
 P8_API_next,
 P8_API_unpack,
 P8_API_pack,
 P8_API_cocreate,
 P8_API_coresume,
 P8_API_costatus,
 P8_API_yield,
 P8_API_load,
 P8_API_ls,
 P8_API_serial,
 P8_API_COUNT
};
#ifdef P8_API_PROFILE
void p8_profile_write(const char *path);
#endif
V p8_api_id(int id,int argc,const V *argv);
V p8_api(const char *name,int argc,const V *argv);
V p8_poke_unpack(int size,V address,V table);
void p8_fail(const char *msg);
void p8_tick(void);
void p8_stack_check(void);
void p8_enter(void);
void p8_leave(void);
void p8_mark(V v);
void p8_collect(void);
void p8_reset(void);
void p8_begin_frame(uint8_t held);
void p8_end_frame(void);
/* Backends provide the frame presented by flip(); default fails explicitly. */
void p8_present(void);
/* Continue a frame from inside flip(): new input and budget, same call stack. */
void p8_flip_frame(uint8_t held);
/* Frame-boundary work for flip(): audio state, collection without compaction. */
void p8_flip_collect(void);
/* Lowest address of the coroutine stack in use, or NULL on the main stack. */
const void *p8_current_stack_low(void);
/* Highest address of the native stack that runs generated code. When set,
 * pool exhaustion triggers a conservative mid-frame collection. */
extern void *p8_stack_top;
/* Screen palette (0x5f10..0x5f1f): 0..15 or 128..143 for each displayed index. */
extern uint8_t p8_screen_palette[16];
/* Extended 32-colour table: 0..15 standard, 16..31 the 128..143 colours. */
extern const uint8_t pico_palette_extended[32][3];
static inline int p8_display_color(int index){int c=p8_screen_palette[index&15];return (c&128)?16+(c&15):(c&15);}
/* run() and extcmd("reset") restart the cartridge: setjmp(p8_guard) returns 2. */
#define P8_RESTART 2
extern bool p8_paused_by_cart;
/* Pause-menu entries registered with menuitem(); index 1..5. */
int p8_menu_count(void);
const char *p8_menu_label(int index);
void p8_menu_select(int index,int buttons);
void p8_text(const char *s,int x,int y,int color);
void p8_overlay_text(const char *s,int x,int y,int c);
const char *p8_describe(V v,char *out,size_t n);
void p8_program_init(void);
void p8_program_update(void);
void p8_program_draw(void);
void p8_program_roots(void);
void p8_program_clear(void);
V p8_debug_global(const char *name);
V p8_env_index(V key);
V p8_inext(void);
V p8_strlong(const char *s,size_t length);
