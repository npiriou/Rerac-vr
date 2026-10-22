# Acceptance

## Original-game integration

- Default Novalis boot enters the original ReRAC application, not the diagnostic terrain loader.
- Original hero, scenery, enemy simulation, weapons, collisions, audio and level-switch plugins run once.
- Two world eye views and matching sky views use the same live entities/materials.
- VR controllers feed the original pad; keyboard and gamepad remain available.
- Original 60 Hz simulation is independent of headset refresh rate.
- Automated desktop smoke requires active ticks, Ratchet meshes and both stereo world/sky views.

## Basic headset check passed

Before the controller remap, the user confirmed that Ratchet and scenery are visible in the headset and that the left stick moves him. OpenXR stereo rendering and live controller input are observed in the log.

## Further physical validation pending

- Inspect depth and completeness across more of the level and both eye images.
- Verify the remapped A/B/X/Y and left/right stick clicks on the real controllers, including jump, wrench, fire, quick-select, Select and Start.
- Check HUD/menu legibility, recenter, tracking loss and session restart.
- Exercise death/respawn and a planet transition in the headset.
- Measure GPU/compositor timing and assess camera comfort.
- Full campaign completion and all upstream unported behaviors are not verified.

The calibration room is a diagnostic only. Its earlier hardware results do not validate the original-game integration.
