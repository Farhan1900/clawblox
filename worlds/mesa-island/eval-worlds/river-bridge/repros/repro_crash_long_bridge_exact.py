#!/usr/bin/env python3

import json
import time
import urllib.request


BASE = 'http://localhost:8099'
SESSION_NAME = 'river-bridge-long-bridge-crash-exact'
SOURCE_RECORDING = '/home/naclo/Code/clawblox/worlds/mesa-island/eval-worlds/river-bridge/results/20260404T154729Z/worlds/world-0/recordings/20260404_154730.clawrec'
SOURCE_RESET = 'tick 133051, 1542.582s'
SOURCE_CUTOFF_TICK = 160489
SOURCE_CUTOFF_TIME_S = 1986.524

ACTIONS = [
    {
        "tick": 133247,
        "time_ms": 1545751,
        "delay_s": 0.0,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.3,
                2.55,
                -16.8
            ]
        }
    },
    {
        "tick": 133371,
        "time_ms": 1547756,
        "delay_s": 2.005,
        "type": "Pickup",
        "data": {
            "position": [
                -40.8,
                0.37,
                -16.8
            ]
        }
    },
    {
        "tick": 133394,
        "time_ms": 1548127,
        "delay_s": 0.371,
        "type": "MoveTo",
        "data": {
            "position": [
                -25.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 133497,
        "time_ms": 1549792,
        "delay_s": 1.665,
        "type": "Place",
        "data": {
            "position": [
                -25.0,
                0.37,
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
        "tick": 133521,
        "time_ms": 1550178,
        "delay_s": 0.386,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.27,
                2.55,
                -16.83
            ]
        }
    },
    {
        "tick": 133612,
        "time_ms": 1551656,
        "delay_s": 1.478,
        "type": "Pickup",
        "data": {
            "position": [
                -40.77,
                0.37,
                -16.83
            ]
        }
    },
    {
        "tick": 133632,
        "time_ms": 1551976,
        "delay_s": 0.32,
        "type": "MoveTo",
        "data": {
            "position": [
                -24.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 133736,
        "time_ms": 1553656,
        "delay_s": 1.68,
        "type": "Place",
        "data": {
            "position": [
                -24.25,
                0.37,
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
        "tick": 133760,
        "time_ms": 1554044,
        "delay_s": 0.388,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.27,
                2.55,
                -16.83
            ]
        }
    },
    {
        "tick": 133868,
        "time_ms": 1555792,
        "delay_s": 1.748,
        "type": "Pickup",
        "data": {
            "position": [
                -40.77,
                0.37,
                -16.83
            ]
        }
    },
    {
        "tick": 133888,
        "time_ms": 1556111,
        "delay_s": 0.319,
        "type": "MoveTo",
        "data": {
            "position": [
                -23.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 133995,
        "time_ms": 1557838,
        "delay_s": 1.727,
        "type": "Place",
        "data": {
            "position": [
                -23.5,
                0.37,
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
        "tick": 134018,
        "time_ms": 1558211,
        "delay_s": 0.373,
        "type": "MoveTo",
        "data": {
            "position": [
                -23.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 134038,
        "time_ms": 1558537,
        "delay_s": 0.326,
        "type": "Place",
        "data": {
            "position": [
                -23.5,
                0.37,
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
        "tick": 134058,
        "time_ms": 1558860,
        "delay_s": 0.323,
        "type": "Place",
        "data": {
            "position": [
                -23.5,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 134085,
        "time_ms": 1559293,
        "delay_s": 0.433,
        "type": "MoveTo",
        "data": {
            "position": [
                -22.0,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 134109,
        "time_ms": 1559686,
        "delay_s": 0.393,
        "type": "Pickup",
        "data": {
            "position": [
                -23.5,
                0.71,
                8.0
            ]
        }
    },
    {
        "tick": 134129,
        "time_ms": 1560007,
        "delay_s": 0.321,
        "type": "MoveTo",
        "data": {
            "position": [
                -22.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 134149,
        "time_ms": 1560326,
        "delay_s": 0.319,
        "type": "Place",
        "data": {
            "position": [
                -22.75,
                0.37,
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
        "tick": 134170,
        "time_ms": 1560669,
        "delay_s": 0.343,
        "type": "MoveTo",
        "data": {
            "position": [
                -38.77,
                2.55,
                -16.75
            ]
        }
    },
    {
        "tick": 134276,
        "time_ms": 1562381,
        "delay_s": 1.712,
        "type": "Pickup",
        "data": {
            "position": [
                -40.27,
                0.37,
                -16.75
            ]
        }
    },
    {
        "tick": 134302,
        "time_ms": 1562802,
        "delay_s": 0.421,
        "type": "MoveTo",
        "data": {
            "position": [
                -22.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 134407,
        "time_ms": 1564500,
        "delay_s": 1.698,
        "type": "Place",
        "data": {
            "position": [
                -22.0,
                0.37,
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
        "tick": 134431,
        "time_ms": 1564888,
        "delay_s": 0.388,
        "type": "MoveTo",
        "data": {
            "position": [
                -40.1,
                2.55,
                -16.8
            ]
        }
    },
    {
        "tick": 134926,
        "time_ms": 1572881,
        "delay_s": 7.993,
        "type": "Pickup",
        "data": {
            "position": [
                -41.6,
                0.37,
                -16.8
            ]
        }
    },
    {
        "tick": 134946,
        "time_ms": 1573209,
        "delay_s": 0.328,
        "type": "MoveTo",
        "data": {
            "position": [
                -21.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135050,
        "time_ms": 1574886,
        "delay_s": 1.677,
        "type": "Place",
        "data": {
            "position": [
                -21.25,
                0.37,
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
        "tick": 135070,
        "time_ms": 1575211,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135091,
        "time_ms": 1575551,
        "delay_s": 0.34,
        "type": "Place",
        "data": {
            "position": [
                -21.25,
                0.37,
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
        "tick": 135114,
        "time_ms": 1575923,
        "delay_s": 0.372,
        "type": "Place",
        "data": {
            "position": [
                -21.25,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 135138,
        "time_ms": 1576307,
        "delay_s": 0.384,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.75,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 135174,
        "time_ms": 1576890,
        "delay_s": 0.583,
        "type": "Pickup",
        "data": {
            "position": [
                -21.25,
                0.71,
                8.0
            ]
        }
    },
    {
        "tick": 135194,
        "time_ms": 1577212,
        "delay_s": 0.322,
        "type": "MoveTo",
        "data": {
            "position": [
                -20.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135220,
        "time_ms": 1577631,
        "delay_s": 0.419,
        "type": "Place",
        "data": {
            "position": [
                -20.5,
                0.37,
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
        "tick": 135241,
        "time_ms": 1577975,
        "delay_s": 0.344,
        "type": "MoveTo",
        "data": {
            "position": [
                -40.1,
                2.55,
                -16.68
            ]
        }
    },
    {
        "tick": 135347,
        "time_ms": 1579684,
        "delay_s": 1.709,
        "type": "Pickup",
        "data": {
            "position": [
                -41.6,
                0.37,
                -16.68
            ]
        }
    },
    {
        "tick": 135370,
        "time_ms": 1580058,
        "delay_s": 0.374,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135476,
        "time_ms": 1581774,
        "delay_s": 1.716,
        "type": "Place",
        "data": {
            "position": [
                -19.75,
                0.37,
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
        "tick": 135500,
        "time_ms": 1582156,
        "delay_s": 0.382,
        "type": "MoveTo",
        "data": {
            "position": [
                -40.09,
                2.55,
                -16.62
            ]
        }
    },
    {
        "tick": 135611,
        "time_ms": 1583954,
        "delay_s": 1.798,
        "type": "Pickup",
        "data": {
            "position": [
                -41.59,
                0.37,
                -16.62
            ]
        }
    },
    {
        "tick": 135631,
        "time_ms": 1584279,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -19.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135755,
        "time_ms": 1586283,
        "delay_s": 2.004,
        "type": "Place",
        "data": {
            "position": [
                -19.0,
                0.37,
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
        "tick": 135776,
        "time_ms": 1586622,
        "delay_s": 0.339,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135799,
        "time_ms": 1586995,
        "delay_s": 0.373,
        "type": "Place",
        "data": {
            "position": [
                -19.0,
                0.37,
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
        "tick": 135819,
        "time_ms": 1587320,
        "delay_s": 0.325,
        "type": "Place",
        "data": {
            "position": [
                -19.0,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 135843,
        "time_ms": 1587706,
        "delay_s": 0.386,
        "type": "MoveTo",
        "data": {
            "position": [
                -17.5,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 135880,
        "time_ms": 1588302,
        "delay_s": 0.596,
        "type": "Pickup",
        "data": {
            "position": [
                -19.0,
                0.71,
                8.0
            ]
        }
    },
    {
        "tick": 135900,
        "time_ms": 1588630,
        "delay_s": 0.328,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 135921,
        "time_ms": 1588967,
        "delay_s": 0.337,
        "type": "Place",
        "data": {
            "position": [
                -18.25,
                0.37,
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
        "tick": 135948,
        "time_ms": 1589406,
        "delay_s": 0.439,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.96,
                2.55,
                -16.65
            ]
        }
    },
    {
        "tick": 136073,
        "time_ms": 1591427,
        "delay_s": 2.021,
        "type": "Pickup",
        "data": {
            "position": [
                -41.46,
                0.37,
                -16.65
            ]
        }
    },
    {
        "tick": 136093,
        "time_ms": 1591745,
        "delay_s": 0.318,
        "type": "MoveTo",
        "data": {
            "position": [
                -17.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 136204,
        "time_ms": 1593543,
        "delay_s": 1.798,
        "type": "Place",
        "data": {
            "position": [
                -17.5,
                0.37,
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
        "tick": 136225,
        "time_ms": 1593880,
        "delay_s": 0.337,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.3,
                2.55,
                -17.6
            ]
        }
    },
    {
        "tick": 136350,
        "time_ms": 1595902,
        "delay_s": 2.022,
        "type": "Pickup",
        "data": {
            "position": [
                -40.8,
                0.37,
                -17.6
            ]
        }
    },
    {
        "tick": 136370,
        "time_ms": 1596225,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -16.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 136498,
        "time_ms": 1598291,
        "delay_s": 2.066,
        "type": "Place",
        "data": {
            "position": [
                -16.75,
                0.37,
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
        "tick": 136524,
        "time_ms": 1598714,
        "delay_s": 0.423,
        "type": "MoveTo",
        "data": {
            "position": [
                -16.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 136547,
        "time_ms": 1599081,
        "delay_s": 0.367,
        "type": "Place",
        "data": {
            "position": [
                -16.75,
                0.37,
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
        "tick": 136567,
        "time_ms": 1599406,
        "delay_s": 0.325,
        "type": "Place",
        "data": {
            "position": [
                -16.75,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 136588,
        "time_ms": 1599744,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -15.25,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 136627,
        "time_ms": 1600371,
        "delay_s": 0.627,
        "type": "Pickup",
        "data": {
            "position": [
                -16.75,
                0.74,
                8.0
            ]
        }
    },
    {
        "tick": 136647,
        "time_ms": 1600701,
        "delay_s": 0.33,
        "type": "MoveTo",
        "data": {
            "position": [
                -16.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 136667,
        "time_ms": 1601025,
        "delay_s": 0.324,
        "type": "Place",
        "data": {
            "position": [
                -16.0,
                0.37,
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
        "tick": 136691,
        "time_ms": 1601411,
        "delay_s": 0.386,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.18,
                2.55,
                -17.62
            ]
        }
    },
    {
        "tick": 136815,
        "time_ms": 1603410,
        "delay_s": 1.999,
        "type": "Pickup",
        "data": {
            "position": [
                -40.68,
                0.37,
                -17.62
            ]
        }
    },
    {
        "tick": 136835,
        "time_ms": 1603735,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -15.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 136958,
        "time_ms": 1605719,
        "delay_s": 1.984,
        "type": "Place",
        "data": {
            "position": [
                -15.25,
                0.37,
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
        "tick": 136979,
        "time_ms": 1606059,
        "delay_s": 0.34,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.19,
                2.55,
                -17.62
            ]
        }
    },
    {
        "tick": 137105,
        "time_ms": 1608092,
        "delay_s": 2.033,
        "type": "Pickup",
        "data": {
            "position": [
                -40.69,
                0.37,
                -17.62
            ]
        }
    },
    {
        "tick": 137125,
        "time_ms": 1608413,
        "delay_s": 0.321,
        "type": "MoveTo",
        "data": {
            "position": [
                -14.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 137251,
        "time_ms": 1610448,
        "delay_s": 2.035,
        "type": "Place",
        "data": {
            "position": [
                -14.5,
                0.37,
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
        "tick": 137274,
        "time_ms": 1610821,
        "delay_s": 0.373,
        "type": "MoveTo",
        "data": {
            "position": [
                -14.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 137294,
        "time_ms": 1611146,
        "delay_s": 0.325,
        "type": "Place",
        "data": {
            "position": [
                -14.5,
                0.37,
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
        "tick": 137314,
        "time_ms": 1611463,
        "delay_s": 0.317,
        "type": "Place",
        "data": {
            "position": [
                -14.5,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 137335,
        "time_ms": 1611805,
        "delay_s": 0.342,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.0,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 137358,
        "time_ms": 1612178,
        "delay_s": 0.373,
        "type": "Pickup",
        "data": {
            "position": [
                -14.5,
                0.74,
                8.0
            ]
        }
    },
    {
        "tick": 137381,
        "time_ms": 1612546,
        "delay_s": 0.368,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 137401,
        "time_ms": 1612873,
        "delay_s": 0.327,
        "type": "Place",
        "data": {
            "position": [
                -13.75,
                0.37,
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
        "tick": 137422,
        "time_ms": 1613211,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -38.68,
                2.55,
                -17.52
            ]
        }
    },
    {
        "tick": 137554,
        "time_ms": 1615344,
        "delay_s": 2.133,
        "type": "Pickup",
        "data": {
            "position": [
                -40.18,
                0.37,
                -17.52
            ]
        }
    },
    {
        "tick": 137574,
        "time_ms": 1615667,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.0,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 137699,
        "time_ms": 1617690,
        "delay_s": 2.023,
        "type": "Place",
        "data": {
            "position": [
                -13.0,
                0.37,
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
        "tick": 137722,
        "time_ms": 1618062,
        "delay_s": 0.372,
        "type": "MoveTo",
        "data": {
            "position": [
                -39.3,
                2.55,
                -18.4
            ]
        }
    },
    {
        "tick": 137862,
        "time_ms": 1620325,
        "delay_s": 2.263,
        "type": "Pickup",
        "data": {
            "position": [
                -40.8,
                0.37,
                -18.4
            ]
        }
    },
    {
        "tick": 137885,
        "time_ms": 1620700,
        "delay_s": 0.375,
        "type": "MoveTo",
        "data": {
            "position": [
                -12.25,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 138024,
        "time_ms": 1622952,
        "delay_s": 2.252,
        "type": "Place",
        "data": {
            "position": [
                -12.25,
                0.37,
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
        "tick": 138044,
        "time_ms": 1623270,
        "delay_s": 0.318,
        "type": "MoveTo",
        "data": {
            "position": [
                -11.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 138064,
        "time_ms": 1623598,
        "delay_s": 0.328,
        "type": "Place",
        "data": {
            "position": [
                -12.25,
                0.37,
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
        "tick": 138084,
        "time_ms": 1623920,
        "delay_s": 0.322,
        "type": "Place",
        "data": {
            "position": [
                -12.25,
                0.37,
                8.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 138105,
        "time_ms": 1624258,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -10.75,
                2.55,
                8.0
            ]
        }
    },
    {
        "tick": 138132,
        "time_ms": 1624694,
        "delay_s": 0.436,
        "type": "Pickup",
        "data": {
            "position": [
                -12.25,
                0.74,
                8.0
            ]
        }
    },
    {
        "tick": 138155,
        "time_ms": 1625066,
        "delay_s": 0.372,
        "type": "MoveTo",
        "data": {
            "position": [
                -11.5,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 138192,
        "time_ms": 1625666,
        "delay_s": 0.6,
        "type": "Place",
        "data": {
            "position": [
                -11.5,
                0.37,
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
        "tick": 138213,
        "time_ms": 1626003,
        "delay_s": 0.337,
        "type": "MoveTo",
        "data": {
            "position": [
                -38.59,
                2.55,
                -17.44
            ]
        }
    },
    {
        "tick": 138348,
        "time_ms": 1628182,
        "delay_s": 2.179,
        "type": "Pickup",
        "data": {
            "position": [
                -40.09,
                0.37,
                -17.44
            ]
        }
    },
    {
        "tick": 138368,
        "time_ms": 1628507,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -10.75,
                2.55,
                2.5
            ]
        }
    },
    {
        "tick": 138497,
        "time_ms": 1630590,
        "delay_s": 2.083,
        "type": "Place",
        "data": {
            "position": [
                -10.75,
                0.37,
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
        "tick": 138518,
        "time_ms": 1630927,
        "delay_s": 0.337,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 139009,
        "time_ms": 1638862,
        "delay_s": 7.935,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 139029,
        "time_ms": 1639187,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 139520,
        "time_ms": 1647132,
        "delay_s": 7.945,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 139542,
        "time_ms": 1647491,
        "delay_s": 0.359,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 140037,
        "time_ms": 1655492,
        "delay_s": 8.001,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 140057,
        "time_ms": 1655816,
        "delay_s": 0.324,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 140547,
        "time_ms": 1663744,
        "delay_s": 7.928,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 140568,
        "time_ms": 1664082,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 141063,
        "time_ms": 1672089,
        "delay_s": 8.007,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 141086,
        "time_ms": 1672464,
        "delay_s": 0.375,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 141573,
        "time_ms": 1680399,
        "delay_s": 7.935,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 141595,
        "time_ms": 1680757,
        "delay_s": 0.358,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 142086,
        "time_ms": 1688731,
        "delay_s": 7.974,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 142109,
        "time_ms": 1689103,
        "delay_s": 0.372,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 142602,
        "time_ms": 1697073,
        "delay_s": 7.97,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 142623,
        "time_ms": 1697412,
        "delay_s": 0.339,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 143116,
        "time_ms": 1705380,
        "delay_s": 7.968,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 143136,
        "time_ms": 1705704,
        "delay_s": 0.324,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 143624,
        "time_ms": 1713598,
        "delay_s": 7.894,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 143645,
        "time_ms": 1713939,
        "delay_s": 0.341,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 144130,
        "time_ms": 1721762,
        "delay_s": 7.823,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 144150,
        "time_ms": 1722084,
        "delay_s": 0.322,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 144631,
        "time_ms": 1729857,
        "delay_s": 7.773,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 144652,
        "time_ms": 1730193,
        "delay_s": 0.336,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 145141,
        "time_ms": 1738096,
        "delay_s": 7.903,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 145161,
        "time_ms": 1738419,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 145644,
        "time_ms": 1746242,
        "delay_s": 7.823,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 145668,
        "time_ms": 1746630,
        "delay_s": 0.388,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 146153,
        "time_ms": 1754476,
        "delay_s": 7.846,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 146176,
        "time_ms": 1754845,
        "delay_s": 0.369,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 146672,
        "time_ms": 1762883,
        "delay_s": 8.038,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 146693,
        "time_ms": 1763221,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 147182,
        "time_ms": 1771154,
        "delay_s": 7.933,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 147202,
        "time_ms": 1771479,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 147699,
        "time_ms": 1779518,
        "delay_s": 8.039,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 147723,
        "time_ms": 1779908,
        "delay_s": 0.39,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 148212,
        "time_ms": 1787812,
        "delay_s": 7.904,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 148232,
        "time_ms": 1788136,
        "delay_s": 0.324,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 148717,
        "time_ms": 1795974,
        "delay_s": 7.838,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 148739,
        "time_ms": 1796329,
        "delay_s": 0.355,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 149223,
        "time_ms": 1804155,
        "delay_s": 7.826,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 149243,
        "time_ms": 1804478,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 149728,
        "time_ms": 1812313,
        "delay_s": 7.835,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 149752,
        "time_ms": 1812701,
        "delay_s": 0.388,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 150238,
        "time_ms": 1820568,
        "delay_s": 7.867,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 150258,
        "time_ms": 1820890,
        "delay_s": 0.322,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 150745,
        "time_ms": 1828771,
        "delay_s": 7.881,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 150766,
        "time_ms": 1829111,
        "delay_s": 0.34,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 151249,
        "time_ms": 1836924,
        "delay_s": 7.813,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 151270,
        "time_ms": 1837265,
        "delay_s": 0.341,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 151752,
        "time_ms": 1845056,
        "delay_s": 7.791,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 151779,
        "time_ms": 1845490,
        "delay_s": 0.434,
        "type": "MoveTo",
        "data": {
            "position": [
                -33.3,
                2.55,
                -24.8
            ]
        }
    },
    {
        "tick": 152265,
        "time_ms": 1853359,
        "delay_s": 7.869,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 152291,
        "time_ms": 1853779,
        "delay_s": 0.42,
        "type": "MoveTo",
        "data": {
            "position": [
                -34.8,
                2.55,
                -26.3
            ]
        }
    },
    {
        "tick": 152782,
        "time_ms": 1861722,
        "delay_s": 7.943,
        "type": "Pickup",
        "data": {
            "position": [
                -34.8,
                0.37,
                -24.8
            ]
        }
    },
    {
        "tick": 152804,
        "time_ms": 1862074,
        "delay_s": 0.352,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 153297,
        "time_ms": 1870063,
        "delay_s": 7.989,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 153317,
        "time_ms": 1870386,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 153800,
        "time_ms": 1878215,
        "delay_s": 7.829,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 153823,
        "time_ms": 1878586,
        "delay_s": 0.371,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 154308,
        "time_ms": 1886456,
        "delay_s": 7.87,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 154328,
        "time_ms": 1886776,
        "delay_s": 0.32,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 154818,
        "time_ms": 1894733,
        "delay_s": 7.957,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 154843,
        "time_ms": 1895138,
        "delay_s": 0.405,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 155334,
        "time_ms": 1903091,
        "delay_s": 7.953,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 155354,
        "time_ms": 1903412,
        "delay_s": 0.321,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 155838,
        "time_ms": 1911240,
        "delay_s": 7.828,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 155859,
        "time_ms": 1911580,
        "delay_s": 0.34,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 156352,
        "time_ms": 1919569,
        "delay_s": 7.989,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 156372,
        "time_ms": 1919894,
        "delay_s": 0.325,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 156862,
        "time_ms": 1927822,
        "delay_s": 7.928,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 156887,
        "time_ms": 1928228,
        "delay_s": 0.406,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 157368,
        "time_ms": 1936011,
        "delay_s": 7.783,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 157389,
        "time_ms": 1936351,
        "delay_s": 0.34,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 157882,
        "time_ms": 1944335,
        "delay_s": 7.984,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 157903,
        "time_ms": 1944674,
        "delay_s": 0.339,
        "type": "MoveTo",
        "data": {
            "position": [
                -37.0,
                2.55,
                18.49
            ]
        }
    },
    {
        "tick": 158389,
        "time_ms": 1952528,
        "delay_s": 7.854,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 158409,
        "time_ms": 1952851,
        "delay_s": 0.323,
        "type": "MoveTo",
        "data": {
            "position": [
                -35.5,
                2.55,
                19.99
            ]
        }
    },
    {
        "tick": 158896,
        "time_ms": 1960726,
        "delay_s": 7.875,
        "type": "Pickup",
        "data": {
            "position": [
                -37.0,
                0.57,
                19.99
            ]
        }
    },
    {
        "tick": 158917,
        "time_ms": 1961064,
        "delay_s": 0.338,
        "type": "MoveTo",
        "data": {
            "position": [
                -25.0,
                2.55,
                2.51
            ]
        }
    },
    {
        "tick": 159412,
        "time_ms": 1969064,
        "delay_s": 8.0,
        "type": "Pickup",
        "data": {
            "position": [
                -25.0,
                0.37,
                5.01
            ]
        }
    },
    {
        "tick": 159436,
        "time_ms": 1969458,
        "delay_s": 0.394,
        "type": "Place",
        "data": {
            "position": [
                -25.0,
                0.37,
                5.01
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 159456,
        "time_ms": 1969778,
        "delay_s": 0.32,
        "type": "MoveTo",
        "data": {
            "position": [
                -18.25,
                2.55,
                2.51
            ]
        }
    },
    {
        "tick": 159950,
        "time_ms": 1977777,
        "delay_s": 7.999,
        "type": "Pickup",
        "data": {
            "position": [
                -18.25,
                0.37,
                5.01
            ]
        }
    },
    {
        "tick": 159971,
        "time_ms": 1978112,
        "delay_s": 0.335,
        "type": "MoveTo",
        "data": {
            "position": [
                -13.0,
                2.55,
                -2.0
            ]
        }
    },
    {
        "tick": 160461,
        "time_ms": 1986070,
        "delay_s": 7.958,
        "type": "Place",
        "data": {
            "position": [
                -13.0,
                0.37,
                0.0
            ],
            "rotation": {
                "pitch": 0,
                "roll": 0,
                "yaw": 0
            }
        }
    },
    {
        "tick": 160489,
        "time_ms": 1986524,
        "delay_s": 0.454,
        "type": "MoveTo",
        "data": {
            "position": [
                15.0,
                2.55,
                0.0
            ]
        }
    }
]


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
    return obs["player"]["attributes"].get("LastActionResult")


def observe(session):
    return reqj(session, "/observe")


def main():
    session = join()
    print("session", session)
    print("recording", SOURCE_RECORDING)
    print("sequence_start", SOURCE_RESET)
    print("sequence_end", {"tick": SOURCE_CUTOFF_TICK, "time_s": SOURCE_CUTOFF_TIME_S})
    print("actions", len(ACTIONS))

    for index, item in enumerate(ACTIONS, start=1):
        delay_s = item["delay_s"]
        if delay_s > 0:
            time.sleep(delay_s)
        result = action(session, item["type"], item["data"])
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

    final_obs = observe(session)
    print("final_player", final_obs["player"]["position"])
    print("final_action", final_obs["player"]["attributes"].get("LastActionResult"))


if __name__ == "__main__":
    main()
