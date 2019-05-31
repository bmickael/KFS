[BITS 32]

extern scheduler_interrupt_handler

segment .text

;; Preemptive schedule beacon
;; Scheduler MUST be not interruptible !
;;
;; +--------+               ^ (to high memory)
;; | SS     | TSS ONLY      |
;; +--------+                    * Illustration of the kernel stack just before IRET
;; | ESP    | TSS ONLY
;; +--------+
;; | EFLAGS |
;; +--------+
;; | CS     |
;; +--------+
;; | EIP    | <---- ESP on the first instruction ---
;; +--------+
;; | DS     |
;; +--------+
;; | ES     |
;; +--------+
;; | FS     |
;; +--------+
;; | GS     |
;; +--------+
;; | REGS   |
;; |    ... |
;; |    ... |
;; +--------+
;; | 0x0    |
;; +--------+ ---> pointer to CpuState Structure (kernel_esp)
global _schedule_next
_schedule_next:
%macro STORE_CONTEXT 0
	; Generate the struct CpuState on the stack :)
	push ds
	push es
	push fs
	push gs
	pushad

	; Push 0x0 for backtrace endpoint
	push dword 0

	; Assign kernel data segments
	mov ax, 0x10
	mov ds, ax
	mov es, ax
	mov fs, ax
	mov gs, ax

	; --- MUST PASS POINTER TO THAT STRUCTURE ---
	push esp
	mov ebp, esp                ; set the backtrace endpoint
%endmacro
	STORE_CONTEXT
	call scheduler_interrupt_handler
	; Set the new stack pointer
	mov esp, eax
schedule_return:
%macro LOAD_CONTEXT 0
	add esp, 4                  ; skip stack reserved field

	; Recover all purpose registers
	popad
	pop gs
	pop fs
	pop es
	pop ds
%endmacro
	LOAD_CONTEXT
	; Return contains now new registers, new eflags, new esp and new eip
	iret

extern _interruptible

; It is identical to the above its mark system as scheduler-interruptible
; This function MUST be used only in a INTGATE context
global _schedule_force_preempt
_schedule_force_preempt:
	STORE_CONTEXT
	call _interruptible
	call scheduler_interrupt_handler
    ; Set the new stack pointer
	mov esp, eax
	LOAD_CONTEXT
	iret

; unsafe extern "C" fn scheduler_exit_resume(process_to_free: Pid, status: i32)
extern scheduler_exit_resume

; fn _exit_resume(new_kernel_esp: u32, process_to_free: Pid, status: i32) -> !;
global _exit_resume
_exit_resume:
	push ebp
	mov ebp, esp

	mov ecx, dword [ebp + 12]   ; get PID of process to free
	mov edx, dword [ebp + 16]   ; get return status of process to free

	; Go to the stack of the new current process
	mov esp, [ebp + 8]

	push edx
	push ecx
	; Free the ressources of the existed process
	call scheduler_exit_resume
	add esp, 8

	jmp schedule_return
