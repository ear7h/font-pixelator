# 1 letter
# 2 bolden, usually 20, set to negative for this font
#
# When inporting, for consistent pixel size, the forms need to be scaled by 12800%
#
# tweaks:
# C --bolden 5
# D --bolden 5
# G --bolden 5
# J --bolden 0
# P --bolden 5
# Q --bolden 10
# S --bolden 5
# S --bolden 18
# Y --bolden 5
cargo run -- test $1 \
	-f source-fonts/LucidaFaxW05-DemiboldItalic.ttf \
	-o glyphs/fax-nouveau/$1.svg \
	--pixels-per-em 16 \
	--bolden $2 \
	--anti-alias \
	--obliquen 0.05

