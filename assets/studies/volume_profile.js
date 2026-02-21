/**
 * Volume Profile
 *
 * Displays volume distribution across price levels for the visible
 * range. Highlights POC (Point of Control) and 70% Value Area.
 * Uses ta.buildProfile() for Rust-optimized profile computation.
 *
 * Output: background profile
 */
indicator("Volume Profile", {
    placement: "background", category: "orderflow",
});

const profile = ta.buildProfile(high, low, buyVolume, sellVolume, tickSize);
plotProfile(profile, { side: "left" });

export {};
