pico-8 cartridge // http://www.pico-8.com
version 41
__lua__
// c-style comment
function sum(...)
 local t,n={...},select('#',...)
 local s=0
 for i=1,n do s+=t[i] end
 return s,n
end
function pass(...) return ... end
function first(a,...) return a,select('#',...) end
local log={}
function check(name,ok) if not ok then printh("FAIL "..name) assert(false,name) end add(log,name) end
function _init()
 cartdata("pocket_test")
 dset(3,42.5) check("dget",dget(3)==42.5)
 poke(0x4300,1,2,3) check("peek",peek(0x4300)==1 and @0x4301==2)
 local a,b,c=peek(0x4300,3) check("peekn",c==3)
 poke2(0x4310,-2) check("peek2",%0x4310==-2)
 poke4(0x4320,1.5) check("peek4",$0x4320==1.5)
 memset(0x4400,7,4) memcpy(0x4500,0x4400,4) check("memcpy",peek(0x4503)==7)
 local s,n=sum(1,2,3,4) check("varargs",s==10 and n==4)
 local x,y,z=pass(7,8,9) check("pass",z==9)
 local f,cnt=first(1,2,3) check("first",cnt==2)
 -- goto continue
 local odd=0
 for i=1,10 do
  if i%2==0 then goto continue end
  odd+=1
  ::continue::
 end
 check("goto",odd==5)
 -- nul strings
 local nul="a\0b"
 check("nul",#nul==3 and ord(nul,2)==0 and sub(nul,3,3)=="b")
 check("chr0",#chr(0,65)==2)
 -- metatables
 local vec={}
 vec.__index=vec
 vec.__add=function(p,q) return setmetatable({x=p.x+q.x},vec) end
 vec.__eq=function(p,q) return p.x==q.x end
 function vec:len() return self.x end
 local v1=setmetatable({x=2},vec)
 local v2=v1+v1
 check("meta",v2:len()==4 and getmetatable(v2)==vec)
 check("eq",setmetatable({x=4},vec)==v2)
 local counter=setmetatable({},{__index=function(t,k) return k*2 end})
 check("index fn",counter[21]==42)
 local callable=setmetatable({},{__call=function(self,a) return a+1 end})
 check("call",callable(4)==5)
 -- custom iterators
 local total=0
 for k,v in next,{10,20,30} do total+=v end
 check("next",total==60)
 local function range(n) local i=0 return function() i+=1 if i<=n then return i end end end
 total=0 for i in range(4) do total+=i end check("closure iter",total==10)
 total=0 for i,v in ipairs({5,6,nil,8}) do total+=v end check("ipairs fn",total==11)
 local it=all({1,2,3}) check("all fn",it()==1 and it()==2)
 check("unpack",select('#',unpack({1,2,3}))==3)
 -- coroutines
 local co=cocreate(function(a)
  local b=yield(a+1)
  local c=yield(b*2)
  return c+100
 end)
 local ok,r=coresume(co,1) check("co1",ok and r==2)
 ok,r=coresume(co,5) check("co2",r==10)
 ok,r=coresume(co,7) check("co3",r==107 and costatus(co)=="dead")
 check("co dead",not coresume(co))
 -- operators
 check("lshr",(0x8000>>>4)==0x0800)
 check("rotl",(1<<>1)==2)
 local q=3 q^=2 check("pow assign",q==9)
 check("xor",(5~3)==6)
 check("intdiv",7\2==3)
 -- menu, palette, fillp
 menuitem(1,"restart",function() run() end)
 pal(1,129,1) check("screen pal",peek(0x5f11)==129)
 local old=fillp(0b1010010110100101.1)
 check("fillp",peek(0x5f31)==0xa5)
 fillp()
 music(0) sfx(3)
 check("stat audio",stat(49)==3 and stat(46)==1 and stat(54)==0)
 poke(0x5f5c,255)
 check("btn p2",btn(0,1)==false)
 check("ls",type(ls())=="table" and #ls()==0)
 check("serial",serial(0x804,0x4300,4)==0)
 check("cstore",cstore(0,0,16)==nil and cstore()==nil)
 check("save",save("x.p8")==nil)
 check("button glyphs",⬅️==0 and ➡️==1 and ⬆️==2 and ⬇️==3 and 🅾️==4 and ❎==5)
 local twice twice,twice=1,2 check("assignment order",twice==1)
 check("nested builtins",mid(0,flr(rnd(1)),max(1,min(5,3)))==0 and select("#",flr(2.5))==1)
 local q=5 do local q=q+1 check("local shadow",q==6) end check("outer local",q==5)
 local p1,p2=q,q+1 p1,p2=p2,p1 check("local swap",p1==6 and p2==5)
 printh("FEATURES_OK "..#log)
end
frames=0
function _update()
 frames+=1
 local t={} for i=1,50 do t[i]={i} end -- garbage every frame
 if frames==3 then
  local n=0
  for i=1,4 do flip() n+=1 end
  check("flip",n==4)
 end
end
function _draw()
 cls(1)
 oval(10,10,50,30,8) ovalfill(60,10,100,40,9)
 rrect(10,50,40,20,4,10) rrectfill(60,50,40,20,6,11)
 fillp(0x5a5a) rectfill(0,90,127,127,0x1c) fillp()
 tline(0,80,127,80,0,0)
 circ(64,64,20,7)
end
__sfx__
000400000c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c0500c050
__music__
00 01020344
