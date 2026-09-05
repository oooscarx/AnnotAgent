# RoboCup football crop verification prompt

Version: 2.0.0

Decide whether the candidate crop contains the RoboCup football itself. A valid football is a
compact ball object, not a white shoe, sock, penalty mark, white field-line intersection, glare,
robot part, person or patch of turf. Do not repair a semantic false positive by tightening its box.
If evidence is ambiguous, request human review.
