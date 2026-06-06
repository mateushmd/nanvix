#!/bin/bash

make run TARGET=x86 MACHINE=qemu-pc IMAGE=build/iso/nanvix.iso LOG_LEVEL=trace GRUB_CFG_SCRIPT=build/iso/boot/grub/grub-net.cfg RUN_WITH_SUDO=yes
