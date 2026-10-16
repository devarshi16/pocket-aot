pico-8 cartridge // Pocket AOT native function regression tests
version 16
__lua__
-- All expected results are asserted by the translated native program.
-- Suggested host run: 2500+ frames, with collection at each frame boundary.
-- One live closure/table cycle and one short-lived cycle per frame; numeric
-- counters wrap at 1000 so long test runs stay inside PICO-8's numeric range.
local top_value=7
local frame_number=0

function read_top()
 return top_value
end

function counter_factory(seed)
 local value=seed
 local function increment(amount)
  value=(value+amount)%1000
  return value
 end
 local function read()
  return value
 end
 return increment,read
end

function cycle_factory(seed)
 local node={value=seed}
 node.self=node
 node.read=function() return node.value end
 node.increment=function(amount)
  node.value=(node.value+amount)%1000
  return node.value
 end
 return node
end

function three_values()
 return 4,5,6
end
function forward_values()
 return three_values()
end
function prefixed_values()
 return 9,three_values()
end
function first_value()
 return (three_values())
end
function no_values()
 return
end
function sum_four(a,b,c,d)
 return a+b+c+d
end

function _init()
 assert(#"⬆️"==1 and ord("⬆️")==148,"P8SCII glyph bytes")
 assert(#"あい"==2 and sub("あい",2)=="い","P8SCII substring")
 assert(type(function() end)=="function","function values have function type")
 assert(read_top()==7,"top-level local capture")
 top_value=0
 frame_number=0
 local increase_one,read_one=counter_factory(10)
 local increase_two,read_two=counter_factory(30)
 assert(increase_one(2)==12,"first factory mutation")
 assert(read_one()==12,"sibling closures share mutable cell")
 assert(read_two()==30,"factories have independent cells")
 assert(increase_two(5)==35 and read_one()==12,"independent factory mutation")
 assert(increase_one(3)==15 and read_one()==15,"shared cell retained")

 local function factorial(n)
  if (n<=1) return 1
  return n*factorial(n-1)
 end
 assert(factorial(5)==120,"local function recursion")
 local recursive
 recursive=function(n)
  if (n<=0) return 0
  return 1+recursive(n-1)
 end
 assert(recursive(6)==6,"anonymous recursive shared cell")

 local receiver_calls=0
 local obj={value=2}
 function obj:bump(amount)
  self.value+=amount
  return self.value
 end
 function obj:sum(a,b,c)
  return self.value+a+b+c
 end
 local function receiver()
  receiver_calls+=1
  return obj
 end
 assert(receiver():bump(3)==5,"method receives self")
 assert(receiver_calls==1,"method receiver evaluated once")
 assert(receiver():sum(three_values())==20,"method tail arguments expand")
 assert(receiver_calls==2,"expanded method receiver evaluated once")

 local effects=0
 local function touch()
  effects+=1
  return 8,9
 end
 assert((false and touch())==false,"and short circuit value")
 assert((true or touch())==true,"or short circuit value")
 assert(effects==0,"short circuit has no effects")
 local short_a,short_b=true and touch()
 assert(short_a==8 and short_b==nil,"logical expression has one result")
 assert(effects==1,"logical expression executes once")

 local remaining={1,2,3,4,5}
 local visited=0
 for value in all(remaining) do
  visited+=value
  if (value%2==0) del(remaining,value)
 end
 assert(visited==15,"all with del visits shifted values")
 assert(#remaining==3 and remaining[1]==1 and remaining[2]==3 and remaining[3]==5,"all with del final array")
 local dictionary={alpha=3,beta=7,[12]=11}
 local pair_count,pair_total=0,0
 for key,value in pairs(dictionary) do
  pair_count+=1
  pair_total+=value
  assert(dictionary[key]==value,"pairs yields actual key")
 end
 assert(pair_count==3 and pair_total==21,"pairs covers mixed keys")
 local callback_total=0
 foreach({2,4,6},function(value) callback_total+=value end)
 assert(callback_total==12,"foreach invokes captured callback")

 local a,b,c,d=three_values()
 assert(a==4 and b==5 and c==6 and d==nil,"assignment expands final call")
 a,b,c=three_values(),10
 assert(a==4 and b==10 and c==nil,"non-final call has one result")
 a,b,c,d=9,three_values()
 assert(a==9 and b==4 and c==5 and d==6,"assignment prefix plus final call")
 a,b,c=forward_values()
 assert(a==4 and b==5 and c==6,"return forwards all final results")
 a,b,c,d=prefixed_values()
 assert(a==9 and b==4 and c==5 and d==6,"return prefix plus final call")
 assert(sum_four(1,three_values())==16,"final argument expands")
 a,b=(three_values())
 assert(a==4 and b==nil,"parenthesized assignment forces one")
 a,b=first_value()
 assert(a==4 and b==nil,"parenthesized return forces one")
 a,b=no_values()
 assert(a==nil and b==nil,"bare return has no values")
 a,b=no_values(),8
 assert(a==nil and b==8,"non-final zero-result call supplies nil")
 local tail={[99]=8,three_values(),}
 assert(#tail==3 and tail[1]==4 and tail[2]==5 and tail[3]==6 and tail[99]==8,"table tail uses implicit array index")
 tail={three_values(),7}
 assert(#tail==2 and tail[1]==4 and tail[2]==7,"non-final table call is scalar")
 tail={(three_values())}
 assert(#tail==1 and tail[1]==4,"parenthesized table tail is scalar")
 tail={no_values()}
 assert(#tail==0,"zero-result table tail adds no item")

 keeper=cycle_factory(0)
 persistent_increment,persistent_read=counter_factory(0)
 assert(keeper.self==keeper and keeper.read()==0,"live closure table cycle")
end

function _update()
 frame_number=(frame_number+1)%1000
 top_value=(top_value+1)%1000
 assert(read_top()==frame_number,"module capture survives collection")
 assert(persistent_increment(1)==frame_number,"factory capture survives collection")
 assert(persistent_read()==frame_number,"sibling capture survives collection")
 assert(keeper.increment(1)==frame_number,"cyclic closure survives collection")
 assert(keeper.self==keeper and keeper.read()==frame_number,"cycle references survive collection")
 local dead=cycle_factory(frame_number)
 assert(dead.self==dead and dead.read()==frame_number,"temporary closure cycle works")
 dead.increment(1)
 assert(dead.read()==(frame_number+1)%1000,"temporary closure shares its cell")
 local callback_total=0
 foreach({1,2,3},function(value) callback_total+=value end)
 assert(callback_total==6,"per-frame callback capture")
end

function _draw()
 cls(1)
 print("native function tests",4,4,7)
 print(frame_number,4,14,10)
end
