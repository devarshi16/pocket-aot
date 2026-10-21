pico-8 cartridge // Original Pocket AOT draw palette regression
version 16
__lua__
-- Independent assertions; no downloaded cartridge code or assets.
function numeric_palette()
 pal() cls(0)
 assert(pal(8,9)==8,"numeric pal returns previous draw mapping")
 pset(2,2,8)
 assert(pget(2,2)==9,"draw mapping affects subsequent pixels")
 local old=pal(8,12,0)
 assert(old==9,"explicit draw palette returns previous mapping")
 pset(3,2,8)
 assert(pget(2,2)==9 and pget(3,2)==12,"mapping leaves existing pixels intact")
 assert(pal(8,old)==12,"numeric pal supports temporary mapping restore")
 pset(4,2,8)
 assert(pget(4,2)==9,"previous mapping can be restored")
 assert(pal(0,5)==16,"previous color zero includes transparency bit")
 assert(palt(0,false)==0x8000,"pal does not change source transparency")
 assert(pal(0,6)==5,"opaque previous mapping omits transparency bit")
 palt(8,true)
 assert(pal(8,11)==25,"other transparent source includes bit four")
 assert(pal(8,3)==27,"mapping changes preserve transparent source bit")
 local swap=pal
 assert(swap(8,2)==19,"first class palette returns previous draw register")
 assert(palt()==0x0080,"numeric mappings leave transparency mask intact")
end

function table_palette()
 pal() cls(0)
 pal({[0]=4}) pset(5,0,0)
 assert(pget(5,0)==4 and pal(0,0)==20,"explicit zero table key preserves transparency")
 pal({[2]=8,[12]=14},0)
 pset(0,0,2) pset(1,0,12) pset(2,0,3)
 assert(pget(0,0)==8 and pget(1,0)==14,"sparse table palette mappings")
 assert(pget(2,0)==3,"sparse palette keeps omitted entries")
 local colors={1,1,5,5,5,6,7,13,6,7,7,6,13,6,7,1}
 pal(colors)
 pset(0,1,0) pset(1,1,1) pset(2,1,15)
 assert(pget(0,1)==1 and pget(1,1)==1 and pget(2,1)==7,"list index sixteen maps color zero")
 assert(pal(0,3)==17,"table palette preserves default color zero transparency")
 palt(1,true)
 pal({[1]=9})
 assert(pal(1,12)==25,"table palette preserves custom source transparency")
 -- Sprite transparency follows the source color, even when it maps elsewhere.
 sset(0,0,0) sset(1,0,1) sset(2,0,2)
 cls(5) sspr(0,0,3,1,0,0)
 assert(pget(0,0)==5 and pget(1,0)==5 and pget(2,0)==1,"sprite transparency before draw remapping")
 pal({})
 pal({[17]=8,[1.5]=8,color=8}) -- ignored keys
 assert(pal(2,4)==1,"empty table leaves current mapping intact")
end

function palette_reset()
 palt(0) pal(2,8) pal(0)
 assert(palt()==0x8000,"pal zero restores default transparency")
 assert(pal(2,9)==2 and pal(0,7)==16,"pal zero restores draw mapping")
 palt(15,true) pal(9,12) pal()
 assert(palt()==0x8000,"pal reset restores default transparency")
 assert(pal(9,1)==9 and pal(15,3)==15,"pal reset restores all draw mappings")
 pal() cls(7)
 sset(0,0,0) sset(1,0,1)
 sspr(0,0,2,1,0,0)
 assert(pget(0,0)==7 and pget(1,0)==1,"reset transparency is visible in sprite blit")
end

function _init()
 numeric_palette()
 table_palette()
 palette_reset()
 kept_palette={[2]=10,[3]=11}
end
function _update()
 pal(kept_palette)
 assert(pal(2,2)==10 and pal(3,3)==11,"palette tables survive collection")
 pal()
end
function _draw()
 cls(1)
 print("palette state checks",4,4,7)
end
