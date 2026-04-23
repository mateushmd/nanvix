# Debugging smoltcp

> **Prerequisite:** You must build Nanvix before debugging it. See [build.md](build.md) for
> instructions.

## 1. Launching the Environment

Run the debug script to build Nanvix and launch QEMU in "wait" mode:

```sh
./scripts/debug-smoltcp.sh
```

QEMU will do nothing but wait for a GDB connection.

## 2. Attaching GDB

In a new terminal, launch GDB pointing to the kernel library:

```sh
gdb bin/kernel.elf
```

Troubleshooting: If GDB warns that `.gdbinit` was "declined," run this command in your shell and restart GDB:

```sh
echo "add-auto-load-safe-path $(pwd)" >> ~/.config/gdb/gdbinit
```

## 3. Setting Breakpoints & Execution

Once inside GDB:

1. Set your targets: Before doing anything else, set the breakpoints you need:
    - `break kmain` (General kernel start)
    - `break pc::init` (Driver initialization)
2. Resume: Type `continue` or `c`.
3. Step-by-Step: Use `next` (`n`) to step over lines or `step` (`s`) to go inside functions.
