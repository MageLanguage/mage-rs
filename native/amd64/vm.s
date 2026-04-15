format ELF64

section '.text' executable

public execute ; (buffer: *mut u8, buffer_size: u64, stack_top: *mut u8) -> u64
execute:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov  r15,   rdx ; stack_top
    mov  r13,   rdi ; buffer base
    mov  r14,   rsi ; buffer size
    lea  rbx,   [rdi + 16] ; bytecode base = buffer + header size
    call patch
    mov  qword  [r15 - 8], rsp ; save system stack to headroom
    lea  rsp,   [r15 - 32] ; switch to mmap'd stack with headroom for root caller frame
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; 0: None
; payload: (none)
code_none:
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; 1: ExitCodeImmutable
; payload: [code:8]
code_exit_code_immutable:
    mov  rax,   qword [rbx]
    mov  rsp,   qword [r15 - 8]
    pop  r15
    pop  r14
    pop  r13
    pop  r12
    pop  rbp
    pop  rbx
    ret

; 2: ExitCodeOffset
; payload: [code_offset:8]
code_exit_code_offset:
    mov  rcx,   qword [rbx]
    mov  rax,   qword [rsp + rcx]
    mov  rsp,   qword [r15 - 8]
    pop  r15
    pop  r14
    pop  r13
    pop  r12
    pop  rbp
    pop  rbx
    ret

; 3: TakeStackSizeImmutable
; payload: [stack_size:8]
code_take_stack_size_immutable:
    sub  rsp,   qword [rbx]
    mov  rax,   qword [rbx + 8]
    add  rbx,   16
    jmp  rax

; 4: FreeStackSizeImmutable
; payload: [stack_size:8]
code_free_stack_size_immutable:
    add  rsp,   qword [rbx]
    mov  rax,   qword [rbx + 8]
    add  rbx,   16
    jmp  rax

; 5: LoadTargetOffsetSourceOffset
; payload: [target_offset:8][source_offset:8]
code_load_target_offset_source_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 16]
    add  rbx,   24
    jmp  rax

; 6: LoadTargetOffsetSourceImmutable
; payload: [target_offset:8][immutable_value:8]
code_load_target_offset_source_immutable:
    mov  rcx,   qword [rbx]
    mov  rax,   qword [rbx + 8]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 16]
    add  rbx,   24
    jmp  rax

; 7: AddTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_add_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    add  rax,   qword [rsp + rdx]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 8: AddTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_add_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    add  rax,   qword [rbx + 16]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 9: SubtractTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_subtract_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    sub  rax,   qword [rsp + rdx]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 10: SubtractTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_subtract_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    sub  rax,   qword [rbx + 16]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 11: MultiplyTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_multiply_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    imul rax,   qword [rsp + rdx]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 12: MultiplyTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_multiply_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    imul rax,   qword [rbx + 16]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 13: DivideTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_divide_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  r8,    qword [rbx + 8]
    mov  rax,   qword [rsp + r8]
    mov  r8,    qword [rbx + 16]
    xor  edx,   edx
    div  qword [rsp + r8]
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 14: DivideTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_divide_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  r8,    qword [rbx + 8]
    mov  rax,   qword [rsp + r8]
    mov  r8,    qword [rbx + 16]
    xor  edx,   edx
    div  r8
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 15: ModuloTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_modulo_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  r8,    qword [rbx + 8]
    mov  rax,   qword [rsp + r8]
    mov  r8,    qword [rbx + 16]
    xor  edx,   edx
    div  qword [rsp + r8]
    mov  qword [rsp + rcx], rdx
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 16: ModuloTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_modulo_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  r8,    qword [rbx + 8]
    mov  rax,   qword [rsp + r8]
    mov  r8,    qword [rbx + 16]
    xor  edx,   edx
    div  r8
    mov  qword [rsp + rcx], rdx
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 17: LessThanTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_less_than_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    setb al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 18: LessThanTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_less_than_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    setb al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 19: GreaterThanTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_greater_than_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    seta al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 20: GreaterThanTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_greater_than_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    seta al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 21: LessThanOrEqualTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_less_than_or_equal_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    setbe al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 22: LessThanOrEqualTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_less_than_or_equal_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    setbe al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 23: GreaterThanOrEqualTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_greater_than_or_equal_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    setae al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 24: GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_greater_than_or_equal_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    setae al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 25: EqualTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_equal_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    sete al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 26: EqualTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_equal_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    sete al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 27: NotEqualTargetOffsetLeftOffsetRightOffset
; payload: [target_offset:8][left_offset:8][right_offset:8]
code_not_equal_target_offset_left_offset_right_offset:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    setne al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 28: NotEqualTargetOffsetLeftOffsetRightImmutable
; payload: [target_offset:8][left_offset:8][right_immutable:8]
code_not_equal_target_offset_left_offset_right_immutable:
    mov  rcx,   qword [rbx]
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    setne al
    movzx eax,  al
    mov  qword [rsp + rcx], rax
    mov  rax,   qword [rbx + 24]
    add  rbx,   32
    jmp  rax

; 29: JumpToImmutable
; payload: [to:8] (patched to absolute pointer)
code_jump_to_immutable:
    mov  rbx,   qword [rbx]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; 30: JumpIfNotConditionOffsetToImmutable
; payload: [condition_offset:8][to:8] (to is patched to absolute pointer)
code_jump_if_not_condition_offset_to_immutable:
    mov  rcx,   qword [rbx]
    mov  rax,   qword [rsp + rcx]
    test rax,   rax
    jnz  .jump_if_not_skip
    mov  rbx,   qword [rbx + 8]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax
.jump_if_not_skip:
    mov  rax,   qword [rbx + 16]
    add  rbx,   24
    jmp  rax

; 31: JumpIfConditionOffsetToImmutable
; payload: [condition_offset:8][to:8] (to is patched to absolute pointer)
code_jump_if_condition_offset_to_immutable:
    mov  rcx,   qword [rbx]
    mov  rax,   qword [rsp + rcx]
    test rax,   rax
    jz   .jump_if_skip
    mov  rbx,   qword [rbx + 8]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax
.jump_if_skip:
    mov  rax,   qword [rbx + 16]
    add  rbx,   24
    jmp  rax

; 32: JumpToOffset
; payload: [to_offset:8] (stack offset containing an absolute bytecode pointer)
code_jump_to_offset:
    mov  rcx,   qword [rbx]
    mov  rbx,   qword [rsp + rcx]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: TakeStackSizeImmutableJumpToImmutable (TakeStackSizeImmutable + JumpToImmutable fused)
; Layout in bytecode: [take_handler:8][stack_size:8][jump_handler:8][to:8]
; After superinstruction patch, take_handler is replaced with the fused handler.
; rbx points to stack_size when handler runs.
code_take_stack_size_immutable_jump_to_immutable:
    sub  rsp,   qword [rbx]       ; stack_size at [rbx]
    mov  rbx,   qword [rbx + 16]  ; to at [rbx+16], skip dead jump opcode at [rbx+8]
    mov  rax,   qword [rbx]       ; read opcode at destination
    add  rbx,   8                  ; advance past opcode
    jmp  rax

; Superinstruction: FreeStackSizeImmutableJumpToImmutable (FreeStackSizeImmutable + JumpToImmutable fused)
; Layout in bytecode: [free_handler:8][stack_size:8][jump_handler:8][to:8]
; After superinstruction patch, free_handler is replaced with the fused handler.
; rbx points to stack_size when handler runs.
code_free_stack_size_immutable_jump_to_immutable:
    add  rsp,   qword [rbx]       ; stack_size at [rbx]
    mov  rbx,   qword [rbx + 16]  ; to at [rbx+16], skip dead jump opcode at [rbx+8]
    mov  rax,   qword [rbx]       ; read opcode at destination
    add  rbx,   8                  ; advance past opcode
    jmp  rax

; Superinstruction: LessThanTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused LessThanTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
; Layout: [handler:8][target:8][left:8][right:8][dead:8][dead:8][to:8]
; rbx+0=target(dead) rbx+8=left rbx+16=right rbx+24=dead rbx+32=dead rbx+40=to rbx+48=next
code_less_than_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jae  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused LessThanTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_less_than_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jae  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused GreaterThanTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
code_greater_than_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jbe  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused GreaterThanTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_greater_than_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jbe  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused LessThanOrEqualTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
code_less_than_or_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    ja   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused LessThanOrEqualTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_less_than_or_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    ja   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused GreaterThanOrEqualTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
code_greater_than_or_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jb   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_greater_than_or_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jb   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: EqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused EqualTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
code_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jne  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: EqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused EqualTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jne  .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: NotEqualTargetOffsetLeftOffsetRightOffsetJumpIfNotConditionOffsetToImmutable
; Fused NotEqualTargetOffsetLeftOffsetRightOffset + JumpIfNotConditionOffsetToImmutable
code_not_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    je   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfNotConditionOffsetToImmutable
; Fused NotEqualTargetOffsetLeftOffsetRightImmutable + JumpIfNotConditionOffsetToImmutable
code_not_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    je   .false
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.false:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; --- Condition + JumpIf superinstructions ---
; Same layout as Condition+JumpIfNot but with inverted branch:
; condition TRUE → jump to [rbx+40], condition FALSE → fall through to [rbx+48]

; Superinstruction: LessThanTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_less_than_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jb   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_less_than_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jb   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_greater_than_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    ja   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_greater_than_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    ja   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_less_than_or_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jbe  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: LessThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_less_than_or_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jbe  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanOrEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_greater_than_or_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jae  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: GreaterThanOrEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_greater_than_or_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jae  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: EqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    je   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: EqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    je   .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: NotEqualTargetOffsetLeftOffsetRightOffsetJumpIfConditionOffsetToImmutable
code_not_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    mov  rdx,   qword [rbx + 16]
    cmp  rax,   qword [rsp + rdx]
    jne  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: NotEqualTargetOffsetLeftOffsetRightImmutableJumpIfConditionOffsetToImmutable
code_not_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable:
    mov  rdx,   qword [rbx + 8]
    mov  rax,   qword [rsp + rdx]
    cmp  rax,   qword [rbx + 16]
    jne  .true
    mov  rax,   qword [rbx + 48]
    add  rbx,   56
    jmp  rax
.true:
    mov  rbx,   qword [rbx + 40]
    mov  rax,   qword [rbx]
    add  rbx,   8
    jmp  rax

; Superinstruction: FreeStackSizeImmutableJumpToOffset (FreeStackSizeImmutable + JumpToOffset fused)
; Layout in bytecode: [free_handler:8][stack_size:8][jump_handler:8][to_offset:8]
; After superinstruction patch, free_handler is replaced with the fused handler.
; rbx points to stack_size when handler runs.
; Must read return address from stack BEFORE freeing the frame.
code_free_stack_size_immutable_jump_to_offset:
    mov  rcx,   qword [rbx + 16]  ; to_offset at [rbx+16] (stack offset of return address)
    mov  r8,    qword [rsp + rcx] ; read return bytecode address from stack before free
    add  rsp,   qword [rbx]       ; stack_size at [rbx] - free frame
    mov  rbx,   r8                ; set bytecode pointer to return address
    mov  rax,   qword [rbx]       ; read opcode at destination
    add  rbx,   8                 ; advance past opcode
    jmp  rax

; Superinstruction: TakeStackSizeImmutableJumpToOffset (TakeStackSizeImmutable + JumpToOffset fused)
; Layout in bytecode: [take_handler:8][stack_size:8][jump_handler:8][to_offset:8]
; After superinstruction patch, take_handler is replaced with the fused handler.
; rbx points to stack_size when handler runs.
; Stack is grown first, then the offset read is relative to the new rsp.
code_take_stack_size_immutable_jump_to_offset:
    sub  rsp,   qword [rbx]       ; stack_size at [rbx]
    mov  rcx,   qword [rbx + 16]  ; to_offset at [rbx+16], skip dead jump opcode at [rbx+8]
    mov  rbx,   qword [rsp + rcx] ; read bytecode address from stack
    mov  rax,   qword [rbx]       ; read opcode at destination
    add  rbx,   8                  ; advance past opcode
    jmp  rax

; Header-driven patcher.
; Reads the 16-byte header at the start of the buffer to locate patch arrays.
;
; Header layout (at r13):
;   [0]  version: u32
;   [4]  patch_offset: u32
;   [8]  superinstruction_offset: u32
;   [12] reserved: u32
;
; Registers on entry:
;   r13 = buffer base (points to header)
;   r14 = buffer size
;   rbx = bytecode base (buffer + 16)
;
; Unified patch entries are u32 values. Bit 0 distinguishes the kind:
;   bit 0 clear -> opcode (replace index with handler address)
;   bit 0 set   -> relocation (add bytecode base to value)
; All bytecode offsets are 8-byte aligned, so bit 0 is always free.
; Superinstruction entries are (opcode_offset: u32, kind: u32) pairs.
patch:
    lea  r8,                   [code_table]

    ; Phase 1: Unified opcode + relocation patching
    ; count = (superinstruction_offset - patch_offset) / 4
    mov  r9d,                  dword [r13 + 4]
    mov  r10d,                 dword [r13 + 8]
    lea  rsi,                  [r13 + r9]
    mov  ecx,                  r10d
    sub  ecx,                  r9d
    shr  ecx,                  2
    test ecx,                  ecx
    jz   .patch_superinstructions
.patch_loop:
    mov  eax,                  dword [rsi]
    test al,                   1
    jnz  .patch_relocation
    ; Opcode: replace index with handler address
    mov  r9,                   qword [rbx + rax]
    mov  r9,                   qword [r8 + r9 * 8]
    mov  qword [rbx + rax],   r9
    add  rsi,                  4
    dec  rcx
    jnz  .patch_loop
    jmp  .patch_superinstructions
.patch_relocation:
    ; Relocation: clear tag bit, add bytecode base
    dec  eax
    add  qword [rbx + rax],   rbx
    add  rsi,                  4
    dec  rcx
    jnz  .patch_loop

    ; Phase 2: Apply superinstruction fusions
    ; count = (buffer_size - superinstruction_offset) / 8
.patch_superinstructions:
    mov  r9d,                  dword [r13 + 8]
    lea  rsi,                  [r13 + r9]
    mov  ecx,                  r14d
    sub  ecx,                  r9d
    shr  ecx,                  3
    test ecx,                  ecx
    jz   .patch_done
    lea  r9,                   [super_code_table]
.patch_super_loop:
    mov  eax,                  dword [rsi]
    mov  edx,                  dword [rsi + 4]
    mov  r10,                  qword [r9 + rdx * 8]
    mov  qword [rbx + rax],   r10
    add  rsi,                  8
    dec  rcx
    jnz  .patch_super_loop

.patch_done:
    ret



section '.data' writeable align 8

public code_table
code_table:
    dq code_none                                                          ; 0
    dq code_exit_code_immutable                                           ; 1
    dq code_exit_code_offset                                              ; 2
    dq code_take_stack_size_immutable                                     ; 3
    dq code_free_stack_size_immutable                                     ; 4
    dq code_load_target_offset_source_offset                              ; 5
    dq code_load_target_offset_source_immutable                           ; 6
    dq code_add_target_offset_left_offset_right_offset                    ; 7
    dq code_add_target_offset_left_offset_right_immutable                 ; 8
    dq code_subtract_target_offset_left_offset_right_offset               ; 9
    dq code_subtract_target_offset_left_offset_right_immutable            ; 10
    dq code_multiply_target_offset_left_offset_right_offset               ; 11
    dq code_multiply_target_offset_left_offset_right_immutable            ; 12
    dq code_divide_target_offset_left_offset_right_offset                 ; 13
    dq code_divide_target_offset_left_offset_right_immutable              ; 14
    dq code_modulo_target_offset_left_offset_right_offset                 ; 15
    dq code_modulo_target_offset_left_offset_right_immutable              ; 16
    dq code_less_than_target_offset_left_offset_right_offset              ; 17
    dq code_less_than_target_offset_left_offset_right_immutable           ; 18
    dq code_greater_than_target_offset_left_offset_right_offset           ; 19
    dq code_greater_than_target_offset_left_offset_right_immutable        ; 20
    dq code_less_than_or_equal_target_offset_left_offset_right_offset     ; 21
    dq code_less_than_or_equal_target_offset_left_offset_right_immutable  ; 22
    dq code_greater_than_or_equal_target_offset_left_offset_right_offset  ; 23
    dq code_greater_than_or_equal_target_offset_left_offset_right_immutable ; 24
    dq code_equal_target_offset_left_offset_right_offset                  ; 25
    dq code_equal_target_offset_left_offset_right_immutable               ; 26
    dq code_not_equal_target_offset_left_offset_right_offset              ; 27
    dq code_not_equal_target_offset_left_offset_right_immutable           ; 28
    dq code_jump_to_immutable                                             ; 29
    dq code_jump_to_offset                                                ; 30
    dq code_jump_if_not_condition_offset_to_immutable                     ; 31
    dq code_jump_if_condition_offset_to_immutable                         ; 32


public super_code_table
super_code_table:
    dq code_take_stack_size_immutable_jump_to_immutable                    ; 0
    dq code_free_stack_size_immutable_jump_to_immutable                    ; 1
    dq code_less_than_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 2
    dq code_less_than_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 3
    dq code_greater_than_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 4
    dq code_greater_than_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 5
    dq code_less_than_or_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 6
    dq code_less_than_or_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 7
    dq code_greater_than_or_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 8
    dq code_greater_than_or_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 9
    dq code_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 10
    dq code_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 11
    dq code_not_equal_target_offset_left_offset_right_offset_jump_if_not_condition_offset_to_immutable ; 12
    dq code_not_equal_target_offset_left_offset_right_immutable_jump_if_not_condition_offset_to_immutable ; 13
    dq code_less_than_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 14
    dq code_less_than_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 15
    dq code_greater_than_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 16
    dq code_greater_than_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 17
    dq code_less_than_or_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 18
    dq code_less_than_or_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 19
    dq code_greater_than_or_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 20
    dq code_greater_than_or_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 21
    dq code_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 22
    dq code_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 23
    dq code_not_equal_target_offset_left_offset_right_offset_jump_if_condition_offset_to_immutable ; 24
    dq code_not_equal_target_offset_left_offset_right_immutable_jump_if_condition_offset_to_immutable ; 25
    dq code_free_stack_size_immutable_jump_to_offset                       ; 26
    dq code_take_stack_size_immutable_jump_to_offset                       ; 27
