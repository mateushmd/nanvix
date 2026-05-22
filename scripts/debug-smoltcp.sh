#!/bin/bash

make debug TARGET=x86 MACHINE=qemu-pc IMAGE=build/iso/nanvix.iso LOG_LEVEL=debug GRUB_CFG_SCRIPT=build/iso/boot/grub/grub-net.cfg
