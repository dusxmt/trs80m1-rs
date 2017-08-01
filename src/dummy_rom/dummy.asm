; To the extent possible under law, Marek Benc has waived all copyright and
; related or neighboring rights to all parts of this dummy system rom.
;
; Full text: http://creativecommons.org/publicdomain/zero/1.0/legalcode
;
;
; This is a dummy rom file that the emulator uses when no system rom image
; is provided by the user.  It displays some static text on the screen.
;
;
; You can use zasm to build this image:
;
;     http://k1.spdns.de/Develop/Projects/zasm/Distributions/
;
;

; Location of the video memory:
video_mem	equ	0x3C00

; Power-on condition:
		org	0
		di

; Copy the new screen content into video memory:
		ld	hl, video_mem
		ld	de, message
		ld	b, 16		; Outer loop iteration count.
outer_loop:	ld	c, 64		; Inner loop iteration count.
inner_loop:	ld	a, (de)
		ld	(hl), a
		inc	de
		inc	hl
		dec	c
		jp	nz, inner_loop	; Loop until we underflow.
		dec	b
		jp	nz, outer_loop	; Same for the outer loop.

stuck:		jp	stuck


; The message to display on the screen:
message:	incbin "message.bin"
