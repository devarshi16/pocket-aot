pico-8 cartridge // Original bitwise API default/conversion assertions
version 16
__lua__
function zero_operand(value)
 assert(band(7,value)==0 and band(value,7)==0,"band zero fallback per operand")
 assert(bor(7,value)==7 and bor(value,7)==7,"bor preserves numeric operand")
 assert(bxor(7,value)==7 and bxor(value,7)==7,"bxor preserves numeric operand")
 assert(bnot(value)==0xffff.ffff,"bnot complements zero fallback")
 assert(shl(value,3)==0 and shr(value,3)==0 and lshr(value,3)==0,"invalid shift value defaults zero")
 assert(shl(7,value)==7 and shr(7,value)==7 and lshr(7,value)==7,"invalid shift amount defaults zero")
 assert(rotl(value,3)==0 and rotr(value,3)==0,"invalid rotate value defaults zero")
 assert(rotl(7,value)==7 and rotr(7,value)==7,"invalid rotate amount defaults zero")
end
function _init()
 zero_operand(nil)
 zero_operand(true)
 zero_operand(false)
 zero_operand({})
 zero_operand(function() return 9 end)
 zero_operand("invalid")
 assert(bor("4",2)==6 and band("0x7",3)==3,"numeric string bitwise operands")
 assert(bxor("0b11",2)==1 and shl("4","1")==8,"radix and shift string conversion")
 assert(bor(nil,1,nil,2)==3 and bxor(3,nil,1)==2,"variadic zero operands")
 assert(bnot(0)==0xffff.ffff and bnot(0xffff.ffff)==0,"numeric complements")
 assert(shr(-1,1)==-0.5 and lshr(-1,1)==0x7fff.8000,"arithmetic versus logical shift")
 assert(shl(1,-1)==0.5 and shr(1,-1)==2,"negative shift direction")
 assert(rotl(0x8000,1)==0x0.0001 and rotr(0x0.0001,1)==0x8000,"rotates preserve fixed bits")
 assert(rotl(3,32)==3 and rotr(3,-32)==3,"rotate counts wrap")
 assert(shl(7,32)==0 and shr(-1,32)==0xffff.ffff,"wide shift bounds")
 local combine=bor
 assert(combine(nil,4)==4,"first class bitwise fallback")
end
function _update()
 zero_operand(nil)
 assert(bor("4",2)==6,"numeric text conversion after collection")
end
function _draw()
 cls(1)
 print("bitwise default checks",4,4,7)
end
