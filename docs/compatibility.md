# Compatibility

This document defines the support boundary. Expand it only with reproducible, authorized observations and tests; a matching brand name is insufficient evidence.

| Product / capability | Status |
|---|---|
| White bulbs exposed as service product ID `12` | Initial implementation; observed successfully on physical bulbs |
| G95G | Name reported for that product ID by the Android app; retail labeling and all hardware revisions have not been independently verified |
| Power, brightness, warm / neutral / cool white | Observed on those bulbs; covered by synthetic protocol tests |
| Other white bulbs or firmware revisions | Unverified |
| RGB bulbs, scenes beyond the white controls, cameras, alarms, plugs | Unsupported |
| Local LAN control, pairing, account creation, desktop login / renewal | Not implemented |
| Local zone grouping / tray power / optional login startup | Implemented; hardware-free tests cover grouping and UI behavior; no new live-hardware tests for zone commands |
| Local scene activation and zone brightness / white | Implemented using existing supported commands; covered by synthetic tests, without new live-hardware validation |
| Cloud rooms / scenes import, timers / schedules | Not implemented |
| Linux | Initial target; other operating systems are untested |

Device enumeration is dynamic. Unsupported product IDs are ignored; Lucine does not probe other accounts, devices or topics. The number of rows is not fixed to an installation.

The cloud must provide TLS endpoints under `iotdreamcatcher.net` or `iotdreamcatcher.net.cn`. IP-only brokers and other domains are rejected. This is an intentional credential protection boundary, not proof that all regions work.

There is no manufacturer's compatibility certification, endorsement or promise of continued service. DreamCatcher Life, DreamCatcher and Chuango are names belonging to their respective owners, used here only to describe observed interoperability.
