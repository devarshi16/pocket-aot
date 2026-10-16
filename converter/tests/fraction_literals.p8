pico-8 cartridge // Original fractional source literal assertions
version 16
__lua__
function check_fraction_literals()
 assert(0x.8==0.5 and 0X.4==0.25,"hex omitted integer part")
 assert(0b.1==0.5 and 0B.01==0.25,"binary omitted integer part")
 assert(0x.0001==0.0000152587890625,"lowest hex fixed bit")
 assert(0b.0000000000000001==0x.0001,"lowest binary fixed bit")
 assert(0x.ffff==0b.1111111111111111,"all fractional bits")
 assert(0x8000.0001==-32768+0x.0001,"signed high bit and low fraction")
 assert(0x10000.8==0x.8,"hex whole part retains wrapping")
 assert(-0x.8==-0.5 and -0B.1==-0.5,"unary minus on radix fraction")
 assert(0x1.==1 and 0b1.==1,"existing empty fraction after integer")
 assert(0x.8.."x"==tostr(0.5).."x" and "y"..0b.1=="y"..tostr(0.5),"concat does not consume point")
 local value=0x.8
 value+=0b.01
 assert(value==0.75,"radix fractions in compound assignment")
 assert(tostr(0x.0001,3)=="0x00000001","lowest bit survives runtime")
 assert(tostr(-0x.0001,3)=="0xffffffff","negative fraction retains exact bits")
end
function _init()
 check_fraction_literals()
end
function _update()
 check_fraction_literals()
end
function _draw()
 cls(1)
 print("fraction literal checks",4,4,7)
end
