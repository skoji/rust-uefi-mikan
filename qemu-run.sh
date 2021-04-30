#!/bin/sh
cp OVMFs/OVMF_VARS.fd .
cp OVMFs/OVMF_CODE.fd .
qemu-system-x86_64 \
    -monitor stdio \
    -drive if=pflash,format=raw,readonly,file=OVMF_CODE.fd \
    -drive if=pflash,format=raw,file=OVMF_VARS.fd \
    -drive if=ide,index=0,media=disk,format=raw,file=LARANJA.img \
    -device nec-usb-xhci,id=xhci \
    -device usb-mouse -device usb-kbd
