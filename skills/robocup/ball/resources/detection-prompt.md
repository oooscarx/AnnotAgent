# RoboCup football detection prompt

Version: 2.0.0

The target is the small football on a RoboCup playing field. Return a tight bounding box containing
only the football itself and no surrounding turf. Never label white shoes, white socks, a penalty
mark, a white-line intersection, glare, or another bright field region as the football. If the
target is not visible enough to localize, return an empty result rather than guessing. The football
may be only 10–20 pixels across, so treat this whole-image result as a coarse proposal that requires
local evidence before geometric refinement.
