format ELF64

section '.text' executable

public context_swap ; (context_link, context_link)
context_swap:
    mov  [rdi + 8],    rbx
    mov  [rdi + 16],   rbp
    mov  [rdi + 24],   r12
    mov  [rdi + 32],   r13
    mov  [rdi + 40],   r14
    mov  [rdi + 48],   r15
    mov  [rdi + 56],   rsp ; registers_ptr.rsp = rsp
    mov  rdi,          rsi
    jmp  context_exit

public context_exit ; (context_link)
context_exit:
    mov  rbx,          [rdi + 8]
    mov  rbp,          [rdi + 16]
    mov  r12,          [rdi + 24]
    mov  r13,          [rdi + 32]
    mov  r14,          [rdi + 40]
    mov  r15,          [rdi + 48]
    mov  rsp,          [rdi + 56] ; rsp = registers_ptr.rsp
    ret
