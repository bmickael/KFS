[BITS 32]
;; This file contains the basic primitives for
;; Interrupt Descriptor Table Register handling.

global _load_idtr
global _get_idtr

;; Loads a specific `struct Idtr` in the Interrupt Descriptor Table Register
_load_idtr:

;; The only parameter is the address of `struct Idtr`
;; passed by the rust _load_idtr routine
	mov	eax, [dword esp + 4]
	lidt	[eax] ;; Load Idtr struct in the idt register
	ret

;; Fills the `struct Idtr` which is passed as an address
_get_idtr:
	mov	eax, [dword esp + 4]
	sidt	[eax]
	ret