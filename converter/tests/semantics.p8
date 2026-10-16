pico-8 cartridge // Pocket AOT semantic regression tests
version 16
__lua__
function side(n)
 trace=trace*10+n
 return n
end
function take(a,b)
 return a*10+b
end
function _init()
 assert(0 or false)
 assert(not nil)
 assert(not false)
 assert(not (true and false))
 assert(0.1==0x0.199a)
 assert(-32768==0x8000)
 assert(1/0==0x7fff.ffff)
 assert(-1/0==-0x7fff.ffff)
 assert(-3%2==1)
 assert(flr(-2.3)==-3)
 assert(ceil(-2.3)==-2)
 assert(sin(0.25)==-1)
 assert(mid(10,2,4)==4)
 trace=0
 assert(take(side(1),side(2))==12)
 assert(trace==12)
 trace=0
 assert((false and side(1))==false)
 assert(true or side(2))
 assert(trace==0)
 local x,y=2,3
 x,y=y,x
 assert(x==3 and y==2)
 local original=4
 do local original=9 assert(original==9) end
 assert(original==4)
 local arr={1,2,3}
 add(arr,4)
 assert(#arr==4 and arr[4]==4)
 del(arr,2)
 assert(#arr==3 and arr[2]==3)
 assert(deli(arr,2)==3)
 assert(#arr==2 and arr[2]==4)
 local obj={name="ok",value=3}
 assert(obj.name=="ok" and obj.value==3)
 obj.self=obj
 obj.value=nil
 assert(obj.value==nil)
 trace=0
 local a={10}
 a[side(1)]+=2
 assert(a[1]==12 and trace==1)
 total=0
 for i=3,1,-1 do total+=i end
 assert(total==6)
 local n=0
 while n<3 do n+=1 end
 assert(n==3)
 repeat local z=n n-=1 until z==1
 assert(n==0)
 if (true) total+=1
 assert(total==7)
 assert("abc"..1=="abc1")
 assert(sub("hello",2,-2)=="ell")
 assert(type(false)=="boolean")
 assert(tonum("3.5")==3.5)
 assert(tonum("oops")==nil)
 assert(#"\135"==1)
 local t=1 t=2 t+=1 assert(t==3)
 counter=0
end
function _update()
 counter+=1
 -- Dead cyclic tables must be reclaimed between frames, as must strings.
 garbage={message="frame "..counter}
 garbage.self=garbage
 local t={}
 for i=1,100 do add(t,i) end
 assert(#t==100)
end
function _draw()
 clip(0,0,1,1)
 cls(1)
 pset(20,20,12)
 assert(pget(20,20)==12)
 sset(0,64,9)
 assert(mget(0,32)==9)
 mset(0,32,7)
 assert(sget(0,64)==7)
 rectfill(2,2,5,5,8)
 assert(pget(4,4)==8)
 print(counter,10,10,7)
 cls(1) pset(0,6,12) cursor(0,128) print("",7) assert(pget(0,0)==12)
 print("a",8) assert(color()==8)
end
