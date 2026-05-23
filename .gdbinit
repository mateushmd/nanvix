# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

layout split
target remote tcp::1234
file bin/kernel.elf
symbol-file bin/kernel.elf
handle SIGSEGV nostop noprint nopass
set scheduler-locking step
set confirm off
focus cmd
set detach-on-fork
#b _do_start
#b _do_ap_start
#b _ap_trampoline
#b kmain
add-symbol-file bin/netd.elf
b src/daemons/netd/src/main.rs:56

define hook-stop
	if $_isvoid ($_exitcode) != 1
		quit
	end

	focus cmd
end
