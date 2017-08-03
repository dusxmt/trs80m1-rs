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
start:		ld	hl, message
		ld	de, video_mem
		ld	bc, 16 * 64
		ldir
		halt

; The message to display on the screen:
message:	incbin "message.bin"
