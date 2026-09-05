# RoboCup football local re-localization prompt

Version: 2.0.0

Inspect the enlarged original-image crop and locate the small RoboCup football itself. Return a
tight bounding box in coordinates relative to this crop, never the full image. Exclude surrounding
turf and reject white shoes, white socks, penalty marks, white-line intersections, glare, robots
and people. The earlier whole-image box is only a nearby coarse proposal and may not cover the
football. If the crop does not contain a clearly localizable football, return an empty result
rather than copying or enlarging the earlier box.
