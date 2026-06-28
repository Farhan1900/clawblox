#!/usr/bin/env python3

import argparse
import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path


BASE = 'http://localhost:8099'
SESSION_NAME = 'river-bridge-110048'
SOURCE_RECORDING = '/home/naclo/Code/clawblox/worlds/mesa-island/eval-worlds/river-bridge/results/20260403T031415Z/worlds/world-0/recordings/20260403_031417.clawrec'
SOURCE_RESET = 'tick 50261, 812.805s'
SOURCE_CUTOFF_TICK = 110048
SOURCE_CUTOFF_TIME_S = 1779.229
WORLD_DIR = Path(__file__).resolve().parents[1]
STARTUP_TIMEOUT_S = 30.0
SETTLE_SECONDS = 0.35
DEFAULT_DELAY_SCALE = float(os.environ.get("REPRO_DELAY_SCALE", "0.0"))
ACTION_TIMEOUT_S = 20.0

ACTIONS = [
    {
        "tick": 52339,
        "time_ms": 846386,
        "delay_s": 0.0,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -14.0
            ]
        }
    },
    {
        "tick": 52837,
        "time_ms": 854434,
        "delay_s": 8.048,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                0.37,
                -14.0
            ]
        }
    },
    {
        "tick": 52899,
        "time_ms": 855438,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -15.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 52992,
        "time_ms": 856944,
        "delay_s": 1.506,
        "type": "Place",
        "data": {
            "position": [
                -15.0,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 53054,
        "time_ms": 857948,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -13.2
            ]
        }
    },
    {
        "tick": 53552,
        "time_ms": 865991,
        "delay_s": 8.043,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                0.37,
                -13.2
            ]
        }
    },
    {
        "tick": 53614,
        "time_ms": 866997,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -14.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 53708,
        "time_ms": 868507,
        "delay_s": 1.51,
        "type": "Place",
        "data": {
            "position": [
                -14.25,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 53770,
        "time_ms": 869511,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -12.4
            ]
        }
    },
    {
        "tick": 54268,
        "time_ms": 877564,
        "delay_s": 8.053,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                0.37,
                -12.4
            ]
        }
    },
    {
        "tick": 54330,
        "time_ms": 878568,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 54393,
        "time_ms": 879577,
        "delay_s": 1.009,
        "type": "Place",
        "data": {
            "position": [
                -13.5,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 54455,
        "time_ms": 880581,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.2,
                0,
                -14.0
            ]
        }
    },
    {
        "tick": 54549,
        "time_ms": 882092,
        "delay_s": 1.511,
        "type": "Pickup",
        "data": {
            "position": [
                -21.2,
                0.37,
                -14.0
            ]
        }
    },
    {
        "tick": 54611,
        "time_ms": 883096,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -12.75,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 54704,
        "time_ms": 884608,
        "delay_s": 1.512,
        "type": "Place",
        "data": {
            "position": [
                -12.75,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 54767,
        "time_ms": 885616,
        "delay_s": 1.008,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.2,
                0,
                -13.2
            ]
        }
    },
    {
        "tick": 54860,
        "time_ms": 887124,
        "delay_s": 1.508,
        "type": "Pickup",
        "data": {
            "position": [
                -21.2,
                0.37,
                -13.2
            ]
        }
    },
    {
        "tick": 54922,
        "time_ms": 888128,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -12.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 55015,
        "time_ms": 889637,
        "delay_s": 1.509,
        "type": "Place",
        "data": {
            "position": [
                -12.0,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 55077,
        "time_ms": 890641,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.2,
                0,
                -12.4
            ]
        }
    },
    {
        "tick": 55140,
        "time_ms": 891646,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -21.2,
                0.37,
                -12.4
            ]
        }
    },
    {
        "tick": 55202,
        "time_ms": 892651,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -11.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 55295,
        "time_ms": 894161,
        "delay_s": 1.51,
        "type": "Place",
        "data": {
            "position": [
                -11.25,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 55357,
        "time_ms": 895166,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.4,
                0,
                -14.0
            ]
        }
    },
    {
        "tick": 55451,
        "time_ms": 896674,
        "delay_s": 1.508,
        "type": "Pickup",
        "data": {
            "position": [
                -20.4,
                0.37,
                -14.0
            ]
        }
    },
    {
        "tick": 55513,
        "time_ms": 897680,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -10.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 55606,
        "time_ms": 899188,
        "delay_s": 1.508,
        "type": "Place",
        "data": {
            "position": [
                -10.5,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 55668,
        "time_ms": 900193,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.4,
                0,
                -13.2
            ]
        }
    },
    {
        "tick": 55762,
        "time_ms": 901701,
        "delay_s": 1.508,
        "type": "Pickup",
        "data": {
            "position": [
                -20.4,
                0.37,
                -13.2
            ]
        }
    },
    {
        "tick": 55824,
        "time_ms": 902706,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -9.75,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 55917,
        "time_ms": 904213,
        "delay_s": 1.507,
        "type": "Place",
        "data": {
            "position": [
                -9.75,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 55979,
        "time_ms": 905217,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.4,
                0,
                -12.4
            ]
        }
    },
    {
        "tick": 56073,
        "time_ms": 906724,
        "delay_s": 1.507,
        "type": "Pickup",
        "data": {
            "position": [
                -20.4,
                0.37,
                -12.4
            ]
        }
    },
    {
        "tick": 56135,
        "time_ms": 907730,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -9.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 56228,
        "time_ms": 909238,
        "delay_s": 1.508,
        "type": "Place",
        "data": {
            "position": [
                -9.0,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 56290,
        "time_ms": 910242,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -14.0
            ]
        }
    },
    {
        "tick": 56789,
        "time_ms": 918298,
        "delay_s": 8.056,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                1.12,
                -14.0
            ]
        }
    },
    {
        "tick": 56851,
        "time_ms": 919302,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 56944,
        "time_ms": 920810,
        "delay_s": 1.508,
        "type": "Place",
        "data": {
            "position": [
                -8.25,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 57006,
        "time_ms": 921814,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -13.2
            ]
        }
    },
    {
        "tick": 57504,
        "time_ms": 929862,
        "delay_s": 8.048,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                1.12,
                -13.2
            ]
        }
    },
    {
        "tick": 57566,
        "time_ms": 930865,
        "delay_s": 1.003,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 57659,
        "time_ms": 932374,
        "delay_s": 1.509,
        "type": "Place",
        "data": {
            "position": [
                -7.5,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 57722,
        "time_ms": 933380,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.0,
                0,
                -12.4
            ]
        }
    },
    {
        "tick": 58219,
        "time_ms": 941424,
        "delay_s": 8.044,
        "type": "Pickup",
        "data": {
            "position": [
                -22.0,
                1.12,
                -12.4
            ]
        }
    },
    {
        "tick": 58282,
        "time_ms": 942429,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -6.75,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 58375,
        "time_ms": 943936,
        "delay_s": 1.507,
        "type": "Place",
        "data": {
            "position": [
                -6.75,
                0.5,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 58816,
        "time_ms": 951068,
        "delay_s": 7.132,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 58878,
        "time_ms": 952074,
        "delay_s": 1.006,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 58940,
        "time_ms": 953079,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 59003,
        "time_ms": 954084,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 59065,
        "time_ms": 955089,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 59127,
        "time_ms": 956094,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 59189,
        "time_ms": 957099,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 59251,
        "time_ms": 958103,
        "delay_s": 1.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 59313,
        "time_ms": 959107,
        "delay_s": 1.004,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 59376,
        "time_ms": 960112,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 59438,
        "time_ms": 961118,
        "delay_s": 1.006,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 59500,
        "time_ms": 962123,
        "delay_s": 1.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 61077,
        "time_ms": 987622,
        "delay_s": 25.499,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 61574,
        "time_ms": 995673,
        "delay_s": 8.051,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 61637,
        "time_ms": 996679,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 62135,
        "time_ms": 1004729,
        "delay_s": 8.05,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 62197,
        "time_ms": 1005736,
        "delay_s": 1.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 62695,
        "time_ms": 1013788,
        "delay_s": 8.052,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 62757,
        "time_ms": 1014793,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 63255,
        "time_ms": 1022840,
        "delay_s": 8.047,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 63317,
        "time_ms": 1023847,
        "delay_s": 1.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 63815,
        "time_ms": 1031892,
        "delay_s": 8.045,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 63877,
        "time_ms": 1032897,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 64376,
        "time_ms": 1040951,
        "delay_s": 8.054,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 64438,
        "time_ms": 1041957,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 64935,
        "time_ms": 1050004,
        "delay_s": 8.047,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 64997,
        "time_ms": 1051010,
        "delay_s": 1.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 65495,
        "time_ms": 1059056,
        "delay_s": 8.046,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 65557,
        "time_ms": 1060061,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 66055,
        "time_ms": 1068106,
        "delay_s": 8.045,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 66117,
        "time_ms": 1069111,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 66615,
        "time_ms": 1077156,
        "delay_s": 8.045,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 66677,
        "time_ms": 1078161,
        "delay_s": 1.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 67175,
        "time_ms": 1086210,
        "delay_s": 8.049,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 67237,
        "time_ms": 1087214,
        "delay_s": 1.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 67735,
        "time_ms": 1095267,
        "delay_s": 8.053,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 69711,
        "time_ms": 1127199,
        "delay_s": 31.932,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 70330,
        "time_ms": 1137204,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 70516,
        "time_ms": 1140209,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 70702,
        "time_ms": 1143213,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 71321,
        "time_ms": 1153218,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 71507,
        "time_ms": 1156223,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 71692,
        "time_ms": 1159229,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 72311,
        "time_ms": 1169234,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 72497,
        "time_ms": 1172237,
        "delay_s": 3.003,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 72683,
        "time_ms": 1175242,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 73302,
        "time_ms": 1185248,
        "delay_s": 10.006,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 73488,
        "time_ms": 1188255,
        "delay_s": 3.007,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 73674,
        "time_ms": 1191265,
        "delay_s": 3.01,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 74293,
        "time_ms": 1201269,
        "delay_s": 10.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 74478,
        "time_ms": 1204273,
        "delay_s": 3.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 74664,
        "time_ms": 1207277,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 75283,
        "time_ms": 1217282,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 75469,
        "time_ms": 1220287,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 75655,
        "time_ms": 1223292,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 76274,
        "time_ms": 1233298,
        "delay_s": 10.006,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 76460,
        "time_ms": 1236303,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 76646,
        "time_ms": 1239310,
        "delay_s": 3.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 77266,
        "time_ms": 1249315,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 77451,
        "time_ms": 1252320,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 77637,
        "time_ms": 1255326,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 78256,
        "time_ms": 1265331,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 78442,
        "time_ms": 1268336,
        "delay_s": 3.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 78628,
        "time_ms": 1271342,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.0
            ]
        }
    },
    {
        "tick": 79247,
        "time_ms": 1281351,
        "delay_s": 10.009,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 79433,
        "time_ms": 1284355,
        "delay_s": 3.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 79619,
        "time_ms": 1287360,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 80238,
        "time_ms": 1297365,
        "delay_s": 10.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.5
            ]
        }
    },
    {
        "tick": 80424,
        "time_ms": 1300369,
        "delay_s": 3.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.5
            ]
        }
    },
    {
        "tick": 82640,
        "time_ms": 1336206,
        "delay_s": 35.837,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 83136,
        "time_ms": 1344212,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 83632,
        "time_ms": 1352218,
        "delay_s": 8.006,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 83818,
        "time_ms": 1355224,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 84313,
        "time_ms": 1363229,
        "delay_s": 8.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -15.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 84809,
        "time_ms": 1371233,
        "delay_s": 8.004,
        "type": "Place",
        "data": {
            "position": [
                -15.0,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 84995,
        "time_ms": 1374238,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 85490,
        "time_ms": 1382245,
        "delay_s": 8.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 85985,
        "time_ms": 1390250,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 86171,
        "time_ms": 1393257,
        "delay_s": 3.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 86667,
        "time_ms": 1401262,
        "delay_s": 8.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -14.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 87162,
        "time_ms": 1409266,
        "delay_s": 8.004,
        "type": "Place",
        "data": {
            "position": [
                -14.25,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 87348,
        "time_ms": 1412271,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 87843,
        "time_ms": 1420277,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                13.5
            ]
        }
    },
    {
        "tick": 88338,
        "time_ms": 1428284,
        "delay_s": 8.007,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 88524,
        "time_ms": 1431289,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 89020,
        "time_ms": 1439296,
        "delay_s": 8.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 89515,
        "time_ms": 1447301,
        "delay_s": 8.005,
        "type": "Place",
        "data": {
            "position": [
                -13.5,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 89701,
        "time_ms": 1450307,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 90196,
        "time_ms": 1458312,
        "delay_s": 8.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 90691,
        "time_ms": 1466316,
        "delay_s": 8.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 90877,
        "time_ms": 1469322,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 91372,
        "time_ms": 1477326,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -12.75,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 91867,
        "time_ms": 1485331,
        "delay_s": 8.005,
        "type": "Place",
        "data": {
            "position": [
                -12.75,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 92053,
        "time_ms": 1488337,
        "delay_s": 3.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 92548,
        "time_ms": 1496343,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 93044,
        "time_ms": 1504348,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 93229,
        "time_ms": 1507351,
        "delay_s": 3.003,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 93724,
        "time_ms": 1515355,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -12.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 94219,
        "time_ms": 1523358,
        "delay_s": 8.003,
        "type": "Place",
        "data": {
            "position": [
                -12.0,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 94405,
        "time_ms": 1526362,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 94900,
        "time_ms": 1534366,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.0,
                0,
                13.5
            ]
        }
    },
    {
        "tick": 95396,
        "time_ms": 1542371,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.0,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 95582,
        "time_ms": 1545376,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 96078,
        "time_ms": 1553381,
        "delay_s": 8.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -11.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 96573,
        "time_ms": 1561386,
        "delay_s": 8.005,
        "type": "Place",
        "data": {
            "position": [
                -11.25,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 96759,
        "time_ms": 1564393,
        "delay_s": 3.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 97254,
        "time_ms": 1572397,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 97749,
        "time_ms": 1580402,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.0
            ]
        }
    },
    {
        "tick": 97935,
        "time_ms": 1583406,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 98430,
        "time_ms": 1591413,
        "delay_s": 8.007,
        "type": "MoveTo",
        "data": {
            "position": [
                -10.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 98926,
        "time_ms": 1599417,
        "delay_s": 8.004,
        "type": "Place",
        "data": {
            "position": [
                -10.5,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 99112,
        "time_ms": 1602425,
        "delay_s": 3.008,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 99607,
        "time_ms": 1610430,
        "delay_s": 8.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 100102,
        "time_ms": 1618435,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                14.5
            ]
        }
    },
    {
        "tick": 100288,
        "time_ms": 1621438,
        "delay_s": 3.003,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 100783,
        "time_ms": 1629446,
        "delay_s": 8.008,
        "type": "MoveTo",
        "data": {
            "position": [
                -9.75,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 101278,
        "time_ms": 1637452,
        "delay_s": 8.006,
        "type": "Place",
        "data": {
            "position": [
                -9.75,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 101463,
        "time_ms": 1640456,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 101959,
        "time_ms": 1648460,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                13.5
            ]
        }
    },
    {
        "tick": 102454,
        "time_ms": 1656465,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -7.5,
                0.19,
                15.0
            ]
        }
    },
    {
        "tick": 102640,
        "time_ms": 1659470,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 103135,
        "time_ms": 1667476,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -9.0,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 103631,
        "time_ms": 1675480,
        "delay_s": 8.004,
        "type": "Place",
        "data": {
            "position": [
                -9.0,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 103817,
        "time_ms": 1678485,
        "delay_s": 3.005,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 104312,
        "time_ms": 1686491,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                12.5
            ]
        }
    },
    {
        "tick": 104807,
        "time_ms": 1694496,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.0
            ]
        }
    },
    {
        "tick": 104993,
        "time_ms": 1697500,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 105488,
        "time_ms": 1705504,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.25,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 105983,
        "time_ms": 1713508,
        "delay_s": 8.004,
        "type": "Place",
        "data": {
            "position": [
                -8.25,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 106168,
        "time_ms": 1716512,
        "delay_s": 3.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                8
            ]
        }
    },
    {
        "tick": 106664,
        "time_ms": 1724518,
        "delay_s": 8.006,
        "type": "MoveTo",
        "data": {
            "position": [
                -8.5,
                0,
                13.0
            ]
        }
    },
    {
        "tick": 107159,
        "time_ms": 1732522,
        "delay_s": 8.004,
        "type": "Pickup",
        "data": {
            "position": [
                -8.5,
                0.57,
                14.5
            ]
        }
    },
    {
        "tick": 107345,
        "time_ms": 1735525,
        "delay_s": 3.003,
        "type": "MoveTo",
        "data": {
            "position": [
                -5.5,
                0,
                3
            ]
        }
    },
    {
        "tick": 107840,
        "time_ms": 1743529,
        "delay_s": 8.004,
        "type": "MoveTo",
        "data": {
            "position": [
                -7.5,
                0,
                3.0
            ]
        }
    },
    {
        "tick": 108335,
        "time_ms": 1751536,
        "delay_s": 8.007,
        "type": "Place",
        "data": {
            "position": [
                -7.5,
                0.94,
                5.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 109553,
        "time_ms": 1771224,
        "delay_s": 19.688,
        "type": "MoveTo",
        "data": {
            "position": [
                -15,
                0,
                3
            ]
        }
    },
    {
        "tick": 110048,
        "time_ms": 1779229,
        "delay_s": 8.005,
        "type": "Pickup",
        "data": {
            "position": [
                -15.01,
                0.37,
                5.0
            ]
        }
    }
]

# Reduced repro:
# keep the opening bridge-build sequence, drop the dead clump probe loop, then
# resume at the first movement back into the useful clump transport sequence.
ACTIONS = [dict(item) for item in (ACTIONS[:48] + ACTIONS[117:])]
ACTIONS[48]["delay_s"] = 0.0


def port_from_base_url(base_url):
    if not base_url.startswith("http://localhost:") and not base_url.startswith("http://127.0.0.1:"):
        raise ValueError(f"only localhost base URLs are supported, got {base_url}")
    return int(base_url.rsplit(":", 1)[1].rstrip("/"))


def find_repo_clawblox_bin():
    current = Path(__file__).resolve()
    for parent in current.parents:
        cargo_toml = parent / "Cargo.toml"
        debug_bin = parent / "target" / "debug" / "clawblox"
        release_bin = parent / "target" / "release" / "clawblox"
        if cargo_toml.exists():
            if debug_bin.exists():
                return debug_bin
            if release_bin.exists():
                return release_bin
    return None


def can_connect():
    try:
        with urllib.request.urlopen(BASE + "/skill.md", timeout=1.5) as response:
            return response.status == 200
    except Exception:
        return False


def wait_for_http(timeout_s=STARTUP_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last_error = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(BASE + "/skill.md", timeout=2) as response:
                if response.status == 200:
                    return
        except Exception as exc:
            last_error = exc
        time.sleep(0.25)
    raise RuntimeError(f"world did not become ready at {BASE} within {timeout_s:.1f}s; last_error={last_error}")


def start_world():
    if can_connect():
        print("world", f"reusing existing server at {BASE}")
        return None
    clawblox_bin = os.environ.get("CLAWBLOX_BIN")
    if not clawblox_bin:
        repo_bin = find_repo_clawblox_bin()
        clawblox_bin = str(repo_bin) if repo_bin is not None else "clawblox"
    port = port_from_base_url(BASE)
    print("world", f"starting local server at {BASE} using {clawblox_bin}")
    process = subprocess.Popen(
        [clawblox_bin, "run", str(WORLD_DIR), "--port", str(port)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    wait_for_http()
    return process


def stop_world(process):
    if process is None:
        return
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def reqj(session, path, method="GET", data=None, timeout=5):
    headers = {"X-Session": session}
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE + path, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def join(name=SESSION_NAME):
    with urllib.request.urlopen(BASE + f"/join?name={name}", data=b"", timeout=5) as response:
        return json.load(response)["session"]


def action(session, kind, data):
    obs = reqj(session, "/input", method="POST", data={"type": kind, "data": data})
    return obs


def observe(session):
    return reqj(session, "/observe")


def last_action_result(obs):
    return obs["player"]["attributes"].get("LastActionResult")


def wait_for_result(session, predicate, timeout_s=ACTION_TIMEOUT_S):
    deadline = time.time() + timeout_s
    last = None
    while time.time() < deadline:
        obs = observe(session)
        last = last_action_result(obs)
        if predicate(last):
            return obs
        time.sleep(0.05)
    raise RuntimeError(f"timeout waiting for action result, last={last!r}")


def wait_for_action_transition(session, previous_result, timeout_s=ACTION_TIMEOUT_S):
    if previous_result is None:
        return observe(session)
    return wait_for_result(session, lambda result: result != previous_result, timeout_s=timeout_s)


def wait_for_action_completion(session, action_type, previous_result, immediate_result):
    if action_type == "MoveTo":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("MoveToFinished"):
                return observe(session)
            return wait_for_result(
                session,
                lambda result: isinstance(result, str) and result.startswith("MoveToFinished"),
            )
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("MoveToFinished"):
            return transitioned_obs
        return wait_for_result(
            session,
            lambda result: isinstance(result, str) and result.startswith("MoveToFinished"),
        )
    if action_type == "Pickup":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("Pickup"):
                return observe(session)
            return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Pickup"))
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("Pickup"):
            return transitioned_obs
        return wait_for_result(
            session,
            lambda result: isinstance(result, str) and result.startswith("Pickup"),
        )
    if action_type == "Place":
        if previous_result is None:
            if isinstance(immediate_result, str) and immediate_result.startswith("Place"):
                return observe(session)
            return wait_for_result(session, lambda result: isinstance(result, str) and result.startswith("Place"))
        transitioned_obs = wait_for_action_transition(session, previous_result)
        transitioned_result = last_action_result(transitioned_obs)
        if isinstance(transitioned_result, str) and transitioned_result.startswith("Place"):
            return transitioned_obs
        return wait_for_result(
            session,
            lambda result: isinstance(result, str) and result.startswith("Place"),
        )
    if action_type == "Reset":
        if previous_result is None:
            return wait_for_result(session, lambda result: result == "ResetSuccess")
        return wait_for_result(session, lambda result: result == "ResetSuccess" and result != previous_result)
    return observe(session)


def count_bridge_line_blocks(obs):
    count = 0
    for entity in obs["world"]["entities"]:
        attrs = entity.get("attributes") or {}
        if attrs.get("CarryKind") != "Block":
            continue
        pos = entity.get("position") or [None, None, None]
        if pos[0] is None:
            continue
        if -15.1 <= pos[0] <= -6.6 and abs(pos[1] - 0.37) <= 0.08 and abs(pos[2] - 5.0) <= 0.06:
            count += 1
    return count


def final_block_positions(obs):
    blocks = []
    for entity in obs["world"]["entities"]:
        attrs = entity.get("attributes") or {}
        if attrs.get("CarryKind") != "Block":
            continue
        pos = entity.get("position") or [None, None, None]
        if pos[0] is None:
            continue
        blocks.append((entity["name"], pos))
    return sorted(blocks)


def print_final_snapshot(obs):
    blocks = final_block_positions(obs)
    lifted = [(name, pos) for name, pos in blocks if pos[1] > 2.0]
    ground_line = [
        (name, pos)
        for name, pos in blocks
        if abs(pos[2] - 5.0) <= 0.06 and abs(pos[1] - 0.37) <= 0.08
    ]

    print("lifted_block_count", len(lifted))
    print("lifted_blocks", lifted)
    print("ground_line_blocks", ground_line)


def parse_args():
    parser = argparse.ArgumentParser(description="Replay the extracted bridge pickup repro.")
    parser.add_argument(
        "--delay-scale",
        type=float,
        default=DEFAULT_DELAY_SCALE,
        help="Scale recorded delays by this factor; default is 0.0 with event-driven waiting.",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    process = start_world()
    try:
        session = join()
        print("session", session)
        print("recording", SOURCE_RECORDING)
        print("sequence_start", SOURCE_RESET)
        print("sequence_end", {"tick": SOURCE_CUTOFF_TICK, "time_s": SOURCE_CUTOFF_TIME_S})
        print("actions", len(ACTIONS))

        previous_result = None
        for index, item in enumerate(ACTIONS, start=1):
            delay_s = item["delay_s"] * args.delay_scale
            if delay_s > 0:
                time.sleep(delay_s)
            action_obs = action(session, item["type"], item["data"])
            immediate_result = last_action_result(action_obs)
            completed_obs = wait_for_action_completion(session, item["type"], previous_result, immediate_result)
            result = last_action_result(completed_obs)
            previous_result = result
            print(
                "step",
                index,
                {
                    "tick": item["tick"],
                    "time_s": round(item["time_ms"] / 1000.0, 3),
                    "type": item["type"],
                    "result": result,
                },
            )

        time.sleep(SETTLE_SECONDS)
        final_obs = observe(session)
        line_count = count_bridge_line_blocks(final_obs)
        print("final_player", final_obs["player"]["position"])
        print("final_action", final_obs["player"]["attributes"].get("LastActionResult"))
        print("bridge_line_blocks_after_pickup", line_count)
        print_final_snapshot(final_obs)
        if line_count > 2:
            raise AssertionError(
                "expected the bonded bridge assembly to leave the ground after pickup, "
                f"but {line_count} blocks are still sitting on the original line"
            )
        print("PASS")
    finally:
        stop_world(process)


if __name__ == "__main__":
    main()
