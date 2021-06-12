# LXZBRA

Emulates a gamepad input device for the purpose of playing Super Mash Bros Melee on Linux with a
keyboard.

![LXZBRA Keyboard Layout](img/layout.png)

The layout is named after the right-hand buttons from pinky inwards. The layout is inspired by
B0XX, with a few modifications:

- The analog stick is not split across the two hands. This makes the layout a bit more intuitive and
  easier to pick up, with no real downside.
- Modifiers for the analog stick have been moved to the left pinky instead. This enables more
  modifiers (up to 4 can be comfortably used, currently only 3 are defined), and also adheres to the
  philosophy of having the fingers rolling from pinky to thumb, since it's often the case that one
  wants to ensure that a modifier is activated before inputting the directional buttons.
- Jump and grab on the right hand have been swapped, because L-cancelled aerials and jump-cancel grab
  both flow from jump to grab. Though the ring finger is the weakest finger, short hopping doesn't
  seem to be problematic (in fact short hopping with characters with 3-frame jumpsquats seem
  completely inconsistent on 4mm-travel mechanical key switches anyway so it's more limited by
  hardware than anything else).
- Y is not intended to be used, the B0XX's philosophy of "not crossing rows" is not applicable on a
  keyboard where the layout of keys are compact. Pressing X and then R to wavedash will not really
  pose any problems.

The analog stick co-ordinates are as follows:

|Modifier|X|Y|Diagonal|
|---|---|---|---|
|Null|1.000|1.000|(0.700, 0.700)|
|X|0.737|0.650|(0.737, 0.313)|
|Y|0.288|0.700|(0.297, 0.700)|
|Shield|0.687|0.650|(0.687, 0.650)|

Mod-X is used to input tilt-attacks (including angling ftilt up or down) and shallow wavedash/upB
angles. Mod-Y is used for steep angles, tilting the shield horizontally for shield dropping, and
turnaround neutral-B. Mod-Shield is used to tilt the shield maximally in each axis.
