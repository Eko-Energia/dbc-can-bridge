"""Simulator Configuration"""

# Simulation settings
DBC_FILE = "perla_bus.dbc"
MODE = "continuous"  # "continuous", "single", or "duration"
DURATION = 30  # Duration in seconds (only for mode="duration")

# Timing
CYCLE_MULTIPLIER = 1.0  # 1.0 = normal, 0.5 = 2x faster, 2.0 = 2x slower (50 = ~5s interval)
CYCLE_VARIATION = 10  # Random variation in cycle times (0-100%)

# Value generation
REALISTIC_MODE = False  # Gradual changes instead of pure random
SMOOTHING_FACTOR = 0.8  # For realistic mode (0.0-1.0)

# Logging
LOG_LEVEL = "warning"  # "debug", "info", "warning", "error"
LOG_FRAMES = False  # Log transmitted frames
LOG_SIGNALS = True  # Log signal values
