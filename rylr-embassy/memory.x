MEMORY {
    /* Pico 2 has 4 MiB QSPI flash. */
    FLASH : ORIGIN = 0x10000000, LENGTH = 4096K
    /* RP2350: 8 banks of 64K = 512K striped SRAM (best general-purpose), plus
       two 4K direct-mapped banks (handy for per-core stacks, etc.). */
    RAM   : ORIGIN = 0x20000000, LENGTH = 512K
    SRAM8 : ORIGIN = 0x20080000, LENGTH = 4K
    SRAM9 : ORIGIN = 0x20081000, LENGTH = 4K
}

SECTIONS {
    /* RP2350 boot ROM scans the first ~4K of flash for an IMAGE_DEF block.
       Place it right after the vector table so it falls within that range. */
    .start_block : ALIGN(4)
    {
        __start_block_addr = .;
        KEEP(*(.start_block));
        KEEP(*(.boot_info));
    } > FLASH
} INSERT AFTER .vector_table;

/* Push .text to start after the boot block, not on top of it. */
_stext = ADDR(.start_block) + SIZEOF(.start_block);

SECTIONS {
    /* Optional: picotool can list binary info found here. */
    .bi_entries : ALIGN(4)
    {
        __bi_entries_start = .;
        KEEP(*(.bi_entries));
        . = ALIGN(4);
        __bi_entries_end = .;
    } > FLASH
} INSERT AFTER .text;

SECTIONS {
    /* End block closes the boot ROM's block loop (the start block's offset
       field, when fixed up by `start_to_end`, points here). */
    .end_block : ALIGN(4)
    {
        __end_block_addr = .;
        KEEP(*(.end_block));
        __flash_binary_end = .;
    } > FLASH
} INSERT AFTER .uninit;

PROVIDE(start_to_end = __end_block_addr - __start_block_addr);
PROVIDE(end_to_start = __start_block_addr - __end_block_addr);
