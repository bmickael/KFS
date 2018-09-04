
[BITS 32]

segment .data

text_color: db 10              ; default to green
cursor_location: dd 0

%include "fonts/alpha.asm"

segment .text

GLOBAL set_cursor_position
GLOBAL set_text_color
GLOBAL putchar

; Indique une nouvelle position en ligne et en colone pour le curseur.
set_cursor_position:
    push ebp
    mov ebp, esp

    mov eax, [ebp + 8]
    mov edx, [ebp + 12]

    shl eax,  3
    shl edx, 14

    add eax, edx
    mov [cursor_location], eax

    pop ebp
ret

set_text_color:
    push ebp
    mov ebp, esp
    mov eax, [ebp + 8]
    mov [text_color], al
    pop ebp
ret

putchar:
    push ebp
    mov ebp, esp
    push ebx
    push esi
    push edi

    mov edi, [cursor_location]

    mov ax, 0x18
    mov es, ax

    test edi, 0x0400
    je .putchar_init
    add edi, 15360

.putchar_init:
    mov eax, [ebp + 8]

    shl eax, 4
    lea esi, [_print_graphical_char_begin + eax]

    mov ax, 0x10
    mov ds, ax

    mov dl, 3
    mov ch, 16                  ; Compteur HEIGHT à 0, il ira jusqu'à 16

.putchar_cycle_heigth:
    lodsb                       ; La première ligne du caractère est chargée
    mov cl, 8                   ; Compteur WIDTH à 0, il ira jusqu'à 8

.putchar_cycle_width:           ; Dispo EAX, EDX et ECX (16 bits forts) (ESI est armé sur le caractère en cours)
    test al, 0x80
    je .tmp
    push eax
    mov al, byte [text_color]
    stosb
    pop eax
    jmp .putchar_return_sequence

 .tmp:
    inc edi

 .putchar_return_sequence:
    shl al, 1
    dec cl
    test cl, cl
    jne .putchar_cycle_width
    add edi, 1016               ; Préparation de EDI pour la prochaine ligne.
    dec ch
    test ch, ch
    jne .putchar_cycle_heigth

    sub edi, 16376
    mov [cursor_location], edi

    pop edi
    pop esi
    pop ebx
    pop ebp
ret
