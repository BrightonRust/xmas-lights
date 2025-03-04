target extended-remote :3333

# print demangled symbols
set print asm-demangle on

# set backtrace limit to not have infinite backtrace loops
set backtrace limit 32

# detect unhandled exceptions, hard faults and panics
# break DefaultHandler
# break HardFault
# break rust_begin_unwind
# # run the next few lines so the panic message is printed immediately
# # the number needs to be adjusted for your panic handler
# commands $bpnum
# next 4
# end

# *try* to stop at the user entry point (it might be gone due to inlining)
# break main

monitor arm semihosting enable

# send captured ITM to the file itm.txt
# (the microcontroller SWO pin must be connected to the programmer SWO pin)
# The final arguement MUST match the core clock frequency in Hz.
#monitor tpiu config internal itm.txt uart off 48000000
monitor tpiu config internal itm.txt uart off 56000000
# The content of itm.txt can be read with:
# itmdump -F -f itm.txt

# # OR: make the microcontroller SWO pin output compatible with UART (8N1)
# # 48000000 must match the core clock frequency
# # 2000000 is the frequency of the SWO pin
# # This will require an external UART adaptor, which must be set to recieve
# # data with a baud rate equal to the final argument.
# monitor tpiu config external uart off 48000000 2000000

# enable ITM port 0
monitor itm port 0 on

load

# start the process but immediately halt the processor
stepi
