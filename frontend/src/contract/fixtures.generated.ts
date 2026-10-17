// Erzeugt von `cargo test contract` (src/contract/fixtures.rs) – nicht von Hand ändern.
import type { AuthOptions } from './generated/AuthOptions';
import type { DeviceLogEvent } from './generated/DeviceLogEvent';
import type { FlightDetail } from './generated/FlightDetail';
import type { FlightSummary } from './generated/FlightSummary';
import type { Health } from './generated/Health';
import type { KioskTokenView } from './generated/KioskTokenView';
import type { LiveEvent } from './generated/LiveEvent';
import type { MapElement } from './generated/MapElement';
import type { Me } from './generated/Me';
import type { RemoteControllerView } from './generated/RemoteControllerView';
import type { SettingsView } from './generated/SettingsView';
import type { SituationSnapshot } from './generated/SituationSnapshot';

export const snapshotEmpty: SituationSnapshot = {
  "active_flights": [],
  "aircraft": [],
  "freshness": {
    "delayed_after_ms": 6000,
    "lost_after_ms": 30000
  },
  "gateways": [],
  "server_time_ms": 1792150000000,
  "streams": [],
  "system": {
    "broker": "connected",
    "invalid_messages": 0,
    "raw_recording": null,
    "unknown_fields": []
  }
};

export const snapshotGatewayOnly: SituationSnapshot = {
  "active_flights": [],
  "aircraft": [],
  "freshness": {
    "delayed_after_ms": 6000,
    "lost_after_ms": 30000
  },
  "gateways": [
    {
      "aircraft_sn": null,
      "battery_percent": 81,
      "label": "RC Plus 1",
      "last_seen_ms": 1792150000000,
      "link": {
        "lte_quality": 3,
        "sdr_quality": 4
      },
      "position": {
        "lat": 52.4296,
        "lon": 10.0097
      },
      "sn": "RCSIM0001",
      "video_sources": []
    }
  ],
  "server_time_ms": 1792150000000,
  "streams": [],
  "system": {
    "broker": "connected",
    "invalid_messages": 0,
    "raw_recording": null,
    "unknown_fields": []
  }
};

export const snapshotFlying: SituationSnapshot = {
  "active_flights": [
    {
      "flight": {
        "aircraft_sn": "ACSIM0001",
        "distance_m": 431.0,
        "end_reason": null,
        "ended_at_ms": null,
        "gateway_sn": "RCSIM0001",
        "id": 7,
        "max_elevation_m": 62.0,
        "started_at_ms": 1792149400000
      },
      "segments": [
        [
          {
            "elevation_m": 2.0,
            "lat": 52.43,
            "lon": 10.01,
            "t_ms": 1792149400000
          },
          {
            "elevation_m": 15.0,
            "lat": 52.4302,
            "lon": 10.0103,
            "t_ms": 1792149402000
          },
          {
            "elevation_m": 40.0,
            "lat": 52.4306,
            "lon": 10.0109,
            "t_ms": 1792149404000
          }
        ],
        [
          {
            "elevation_m": 61.0,
            "lat": 52.4315,
            "lon": 10.012,
            "t_ms": 1792149430000
          },
          {
            "elevation_m": 62.0,
            "lat": 52.4321,
            "lon": 10.0134,
            "t_ms": 1792149432000
          }
        ]
      ]
    }
  ],
  "aircraft": [
    {
      "flight_id": 7,
      "gateway_sn": "RCSIM0001",
      "last_seen_ms": 1792150000000,
      "phase": "airborne",
      "sn": "ACSIM0001",
      "telemetry": {
        "battery": {
          "percent": 63,
          "remaining_flight_s": 1100,
          "return_home_percent": 22
        },
        "elevation_takeoff_m": 62.0,
        "gnss": {
          "fix": "fixed",
          "gps_satellites": 21,
          "quality": 5,
          "rtk_satellites": 18
        },
        "heading_deg": 274.0,
        "height_ellipsoid_m": 171.4,
        "home": {
          "lat": 52.43,
          "lon": 10.01
        },
        "home_distance_m": 412.0,
        "horizontal_speed_mps": 8.4,
        "mode": {
          "code": 3,
          "label": "manueller Flug"
        },
        "position": {
          "lat": 52.4321,
          "lon": 10.0134
        },
        "vertical_speed_mps": 0.0
      }
    }
  ],
  "freshness": {
    "delayed_after_ms": 6000,
    "lost_after_ms": 30000
  },
  "gateways": [
    {
      "aircraft_sn": "ACSIM0001",
      "battery_percent": 81,
      "label": "RC Plus 1",
      "last_seen_ms": 1792150000000,
      "link": {
        "lte_quality": 3,
        "sdr_quality": 4
      },
      "position": {
        "lat": 52.4296,
        "lon": 10.0097
      },
      "sn": "RCSIM0001",
      "video_sources": [
        {
          "camera_index": "61-0-0",
          "camera_label": "H20N",
          "lens": "wide",
          "switchable_lenses": [
            "wide",
            "zoom",
            "thermal"
          ],
          "video_id": "ACSIM0001/61-0-0/normal-0"
        },
        {
          "camera_index": "39-0-7",
          "camera_label": "FPV",
          "lens": "normal",
          "switchable_lenses": [],
          "video_id": "ACSIM0001/39-0-7/normal-0"
        }
      ]
    }
  ],
  "server_time_ms": 1792150001000,
  "streams": [
    {
      "error": null,
      "gateway_sn": "RCSIM0001",
      "lens": "wide",
      "phase": "live",
      "updated_at_ms": 1792149410000,
      "video_id": "ACSIM0001/61-0-0/normal-0"
    }
  ],
  "system": {
    "broker": "connected",
    "invalid_messages": 0,
    "raw_recording": "20261016T120000Z.jsonl",
    "unknown_fields": [
      {
        "count": 3,
        "key": "aircraft.wind_speed_extra"
      }
    ]
  }
};

export const liveEvents: LiveEvent[] = [
  {
    "snapshot": {
      "active_flights": [
        {
          "flight": {
            "aircraft_sn": "ACSIM0001",
            "distance_m": 431.0,
            "end_reason": null,
            "ended_at_ms": null,
            "gateway_sn": "RCSIM0001",
            "id": 7,
            "max_elevation_m": 62.0,
            "started_at_ms": 1792149400000
          },
          "segments": [
            [
              {
                "elevation_m": 2.0,
                "lat": 52.43,
                "lon": 10.01,
                "t_ms": 1792149400000
              },
              {
                "elevation_m": 15.0,
                "lat": 52.4302,
                "lon": 10.0103,
                "t_ms": 1792149402000
              },
              {
                "elevation_m": 40.0,
                "lat": 52.4306,
                "lon": 10.0109,
                "t_ms": 1792149404000
              }
            ],
            [
              {
                "elevation_m": 61.0,
                "lat": 52.4315,
                "lon": 10.012,
                "t_ms": 1792149430000
              },
              {
                "elevation_m": 62.0,
                "lat": 52.4321,
                "lon": 10.0134,
                "t_ms": 1792149432000
              }
            ]
          ]
        }
      ],
      "aircraft": [
        {
          "flight_id": 7,
          "gateway_sn": "RCSIM0001",
          "last_seen_ms": 1792150000000,
          "phase": "airborne",
          "sn": "ACSIM0001",
          "telemetry": {
            "battery": {
              "percent": 63,
              "remaining_flight_s": 1100,
              "return_home_percent": 22
            },
            "elevation_takeoff_m": 62.0,
            "gnss": {
              "fix": "fixed",
              "gps_satellites": 21,
              "quality": 5,
              "rtk_satellites": 18
            },
            "heading_deg": 274.0,
            "height_ellipsoid_m": 171.4,
            "home": {
              "lat": 52.43,
              "lon": 10.01
            },
            "home_distance_m": 412.0,
            "horizontal_speed_mps": 8.4,
            "mode": {
              "code": 3,
              "label": "manueller Flug"
            },
            "position": {
              "lat": 52.4321,
              "lon": 10.0134
            },
            "vertical_speed_mps": 0.0
          }
        }
      ],
      "freshness": {
        "delayed_after_ms": 6000,
        "lost_after_ms": 30000
      },
      "gateways": [
        {
          "aircraft_sn": "ACSIM0001",
          "battery_percent": 81,
          "label": "RC Plus 1",
          "last_seen_ms": 1792150000000,
          "link": {
            "lte_quality": 3,
            "sdr_quality": 4
          },
          "position": {
            "lat": 52.4296,
            "lon": 10.0097
          },
          "sn": "RCSIM0001",
          "video_sources": [
            {
              "camera_index": "61-0-0",
              "camera_label": "H20N",
              "lens": "wide",
              "switchable_lenses": [
                "wide",
                "zoom",
                "thermal"
              ],
              "video_id": "ACSIM0001/61-0-0/normal-0"
            },
            {
              "camera_index": "39-0-7",
              "camera_label": "FPV",
              "lens": "normal",
              "switchable_lenses": [],
              "video_id": "ACSIM0001/39-0-7/normal-0"
            }
          ]
        }
      ],
      "server_time_ms": 1792150001000,
      "streams": [
        {
          "error": null,
          "gateway_sn": "RCSIM0001",
          "lens": "wide",
          "phase": "live",
          "updated_at_ms": 1792149410000,
          "video_id": "ACSIM0001/61-0-0/normal-0"
        }
      ],
      "system": {
        "broker": "connected",
        "invalid_messages": 0,
        "raw_recording": "20261016T120000Z.jsonl",
        "unknown_fields": [
          {
            "count": 3,
            "key": "aircraft.wind_speed_extra"
          }
        ]
      }
    },
    "type": "snapshot"
  },
  {
    "gateway": {
      "aircraft_sn": "ACSIM0001",
      "battery_percent": 81,
      "label": "RC Plus 1",
      "last_seen_ms": 1792150000000,
      "link": {
        "lte_quality": 3,
        "sdr_quality": 4
      },
      "position": {
        "lat": 52.4296,
        "lon": 10.0097
      },
      "sn": "RCSIM0001",
      "video_sources": [
        {
          "camera_index": "61-0-0",
          "camera_label": "H20N",
          "lens": "wide",
          "switchable_lenses": [
            "wide",
            "zoom",
            "thermal"
          ],
          "video_id": "ACSIM0001/61-0-0/normal-0"
        },
        {
          "camera_index": "39-0-7",
          "camera_label": "FPV",
          "lens": "normal",
          "switchable_lenses": [],
          "video_id": "ACSIM0001/39-0-7/normal-0"
        }
      ]
    },
    "type": "gateway"
  },
  {
    "aircraft": {
      "flight_id": 7,
      "gateway_sn": "RCSIM0001",
      "last_seen_ms": 1792150000000,
      "phase": "airborne",
      "sn": "ACSIM0001",
      "telemetry": {
        "battery": {
          "percent": 63,
          "remaining_flight_s": 1100,
          "return_home_percent": 22
        },
        "elevation_takeoff_m": 62.0,
        "gnss": {
          "fix": "fixed",
          "gps_satellites": 21,
          "quality": 5,
          "rtk_satellites": 18
        },
        "heading_deg": 274.0,
        "height_ellipsoid_m": 171.4,
        "home": {
          "lat": 52.43,
          "lon": 10.01
        },
        "home_distance_m": 412.0,
        "horizontal_speed_mps": 8.4,
        "mode": {
          "code": 3,
          "label": "manueller Flug"
        },
        "position": {
          "lat": 52.4321,
          "lon": 10.0134
        },
        "vertical_speed_mps": 0.0
      }
    },
    "type": "aircraft"
  },
  {
    "flight_id": 7,
    "point": {
      "elevation_m": 62.0,
      "lat": 52.4323,
      "lon": 10.0131,
      "t_ms": 1792150002000
    },
    "segment": 1,
    "type": "track_point"
  },
  {
    "flight": {
      "aircraft_sn": "ACSIM0001",
      "distance_m": 431.0,
      "end_reason": null,
      "ended_at_ms": null,
      "gateway_sn": "RCSIM0001",
      "id": 7,
      "max_elevation_m": 62.0,
      "started_at_ms": 1792149400000
    },
    "type": "flight_started"
  },
  {
    "flight": {
      "aircraft_sn": "ACSIM0001",
      "distance_m": 2310.0,
      "end_reason": "landed",
      "ended_at_ms": 1792150480000,
      "gateway_sn": "RCSIM0001",
      "id": 7,
      "max_elevation_m": 85.0,
      "started_at_ms": 1792149400000
    },
    "type": "flight_ended"
  },
  {
    "stream": {
      "error": "Pilot 2 lehnt ab: Livestream ist in Pilot 2 nicht aktiviert (513011).",
      "gateway_sn": "RCSIM0001",
      "lens": "wide",
      "phase": "failed",
      "updated_at_ms": 1792149410000,
      "video_id": "ACSIM0001/61-0-0/normal-0"
    },
    "type": "stream"
  },
  {
    "system": {
      "broker": "connected",
      "invalid_messages": 0,
      "raw_recording": "20261016T120000Z.jsonl",
      "unknown_fields": [
        {
          "count": 3,
          "key": "aircraft.wind_speed_extra"
        }
      ]
    },
    "type": "system"
  },
  {
    "elements": [
      {
        "author": "Gruppenführer I&K",
        "clamp_to_ground": true,
        "color": "#FFBB00",
        "created_at_ms": 1792149700000,
        "id": "3f0c1a52-6d1e-4c39-9a51-0b7e2f4d8c11",
        "kind": "polygon",
        "name": "Suchgebiet Nord",
        "origin": "web",
        "points": [
          {
            "alt_m": null,
            "lat": 52.433,
            "lon": 10.008
          },
          {
            "alt_m": null,
            "lat": 52.433,
            "lon": 10.016
          },
          {
            "alt_m": null,
            "lat": 52.4305,
            "lon": 10.016
          },
          {
            "alt_m": null,
            "lat": 52.4305,
            "lon": 10.008
          }
        ],
        "updated_at_ms": 1792149700000
      },
      {
        "author": "RC Plus 1",
        "clamp_to_ground": false,
        "color": "#E23C39",
        "created_at_ms": 1792149940000,
        "id": "9b2e4f60-1c3d-4e5f-8a7b-6c5d4e3f2a10",
        "kind": "point",
        "name": "Pilot 1",
        "origin": "pilot",
        "points": [
          {
            "alt_m": 84.2,
            "lat": 52.4318,
            "lon": 10.0124
          }
        ],
        "updated_at_ms": 1792149940000
      }
    ],
    "type": "map_elements"
  },
  {
    "element": {
      "author": "RC Plus 1",
      "clamp_to_ground": false,
      "color": "#E23C39",
      "created_at_ms": 1792149940000,
      "id": "9b2e4f60-1c3d-4e5f-8a7b-6c5d4e3f2a10",
      "kind": "point",
      "name": "Pilot 1",
      "origin": "pilot",
      "points": [
        {
          "alt_m": 84.2,
          "lat": 52.4318,
          "lon": 10.0124
        }
      ],
      "updated_at_ms": 1792149940000
    },
    "type": "map_element"
  },
  {
    "id": "3f0c1a52-6d1e-4c39-9a51-0b7e2f4d8c11",
    "type": "map_element_deleted"
  }
];

export const deviceLog: DeviceLogEvent[] = [
  {
    "entries": [
      {
        "at_ms": 1792150001000,
        "device_sn": "RCSIM0001",
        "level": "warn",
        "method": null,
        "payload": null,
        "seq": 1,
        "source": "pilot",
        "summary": "Verbindung zu Suchlage: getrennt",
        "topic": null
      },
      {
        "at_ms": 1792150002000,
        "device_sn": "RCSIM0001",
        "level": "info",
        "method": "live_start_push",
        "payload": "{\"method\":\"live_start_push\",\"data\":{\"result\":0}}",
        "seq": 2,
        "source": "server",
        "summary": "Befehl live_start_push",
        "topic": "thing/product/RCSIM0001/services"
      }
    ],
    "type": "history"
  },
  {
    "entry": {
      "at_ms": 1792150003000,
      "device_sn": "RCSIM0001",
      "level": "error",
      "method": "live_start_push",
      "payload": "{\"method\":\"live_start_push\",\"data\":{\"result\":0}}",
      "seq": 3,
      "source": "system",
      "summary": "live_start_push: keine Antwort in 10 s",
      "topic": null
    },
    "type": "entry"
  },
  {
    "count": 4,
    "type": "missed"
  }
];

export const flightList: FlightSummary[] = [
  {
    "aircraft_sn": "ACSIM0001",
    "distance_m": 431.0,
    "end_reason": null,
    "ended_at_ms": null,
    "gateway_sn": "RCSIM0001",
    "id": 7,
    "max_elevation_m": 62.0,
    "started_at_ms": 1792149400000
  },
  {
    "aircraft_sn": "ACSIM0001",
    "distance_m": 2310.0,
    "end_reason": "landed",
    "ended_at_ms": 1792143880000,
    "gateway_sn": "RCSIM0001",
    "id": 6,
    "max_elevation_m": 85.0,
    "started_at_ms": 1792142800000
  },
  {
    "aircraft_sn": "ACSIM0001",
    "distance_m": 2310.0,
    "end_reason": "landed",
    "ended_at_ms": 1792064680000,
    "gateway_sn": "RCSIM0001",
    "id": 5,
    "max_elevation_m": 85.0,
    "started_at_ms": 1792063600000
  }
];

export const flightDetail: FlightDetail = {
  "flight": {
    "aircraft_sn": "ACSIM0001",
    "distance_m": 2310.0,
    "end_reason": "landed",
    "ended_at_ms": 1792143880000,
    "gateway_sn": "RCSIM0001",
    "id": 6,
    "max_elevation_m": 85.0,
    "started_at_ms": 1792142800000
  },
  "segments": [
    [
      {
        "elevation_m": 2.0,
        "lat": 52.43,
        "lon": 10.01,
        "t_ms": 1792149400000
      },
      {
        "elevation_m": 15.0,
        "lat": 52.4302,
        "lon": 10.0103,
        "t_ms": 1792149402000
      },
      {
        "elevation_m": 40.0,
        "lat": 52.4306,
        "lon": 10.0109,
        "t_ms": 1792149404000
      }
    ],
    [
      {
        "elevation_m": 61.0,
        "lat": 52.4315,
        "lon": 10.012,
        "t_ms": 1792149430000
      },
      {
        "elevation_m": 62.0,
        "lat": 52.4321,
        "lon": 10.0134,
        "t_ms": 1792149432000
      }
    ]
  ],
  "video": {
    "download_url": "/api/flights/6/video?download=1",
    "play_url": "/api/flights/6/video"
  }
};

export const settingsView: SettingsView = {
  "broker": {
    "admin_username": "suchlage-admin",
    "has_admin_password": true,
    "host": "mqtt",
    "port": 1883,
    "rc_url": "tcp://100.64.0.10:1883"
  },
  "dji": {
    "app_id": "123456",
    "has_app_key": true,
    "has_license": true,
    "workspace_id": "00000000-0000-4000-8000-000000000001"
  },
  "general": {
    "auto_live_on_takeoff": true,
    "record_raw_mqtt": true
  },
  "media": {
    "hls_url": "http://media:8888",
    "playback_url": "http://media:9996",
    "rtmp_url": "rtmp://100.64.0.10:1935",
    "webrtc_url": "http://media:8889"
  },
  "oidc": {
    "client_id": "suchlage",
    "has_client_secret": true,
    "issuer": "https://id.example.org",
    "redirect_url": "https://suchlage.example.org/api/auth/oidc/callback"
  },
  "photo_storage": {
    "access_key_id": "GK31c2f218a2e44f485b94239e",
    "admin_url": "http://fotos:3903",
    "bucket": "suchlage-fotos",
    "has_admin_token": true,
    "has_secret_access_key": true,
    "rc_url": "http://100.64.0.10:3900",
    "region": "garage",
    "s3_url": "http://fotos:3900"
  }
};

export const settingsEmpty: SettingsView = {
  "broker": null,
  "dji": null,
  "general": {
    "auto_live_on_takeoff": true,
    "record_raw_mqtt": true
  },
  "media": null,
  "oidc": null,
  "photo_storage": null
};

export const remoteControllers: RemoteControllerView[] = [
  {
    "id": 1,
    "label": "RC Plus 1",
    "paired_at_ms": 1792063600000,
    "sn": "RCSIM0001"
  },
  {
    "id": 2,
    "label": "RC Plus Reserve",
    "paired_at_ms": null,
    "sn": null
  }
];

export const kioskTokens: KioskTokenView[] = [
  {
    "created_at_ms": 1792146400000,
    "id": 1,
    "label": "FüKw Großbild"
  }
];

export const mapElements: MapElement[] = [
  {
    "author": "Gruppenführer I&K",
    "clamp_to_ground": true,
    "color": "#FFBB00",
    "created_at_ms": 1792149700000,
    "id": "3f0c1a52-6d1e-4c39-9a51-0b7e2f4d8c11",
    "kind": "polygon",
    "name": "Suchgebiet Nord",
    "origin": "web",
    "points": [
      {
        "alt_m": null,
        "lat": 52.433,
        "lon": 10.008
      },
      {
        "alt_m": null,
        "lat": 52.433,
        "lon": 10.016
      },
      {
        "alt_m": null,
        "lat": 52.4305,
        "lon": 10.016
      },
      {
        "alt_m": null,
        "lat": 52.4305,
        "lon": 10.008
      }
    ],
    "updated_at_ms": 1792149700000
  },
  {
    "author": "RC Plus 1",
    "clamp_to_ground": false,
    "color": "#E23C39",
    "created_at_ms": 1792149940000,
    "id": "9b2e4f60-1c3d-4e5f-8a7b-6c5d4e3f2a10",
    "kind": "point",
    "name": "Pilot 1",
    "origin": "pilot",
    "points": [
      {
        "alt_m": 84.2,
        "lat": 52.4318,
        "lon": 10.0124
      }
    ],
    "updated_at_ms": 1792149940000
  }
];

export const meAdmin: Me = {
  "display_name": "Erika Mustermann",
  "role": "admin"
};

export const meOperator: Me = {
  "display_name": "Gruppenführer I&K",
  "role": "operator"
};

export const meKiosk: Me = {
  "display_name": "FüKw Großbild",
  "role": "kiosk"
};

export const authOptionsDev: AuthOptions = {
  "dev_login": true,
  "oidc": false,
  "redirect_url": "https://suchlage.example.org/api/auth/oidc/callback",
  "setup_required": false
};

export const authOptionsSetup: AuthOptions = {
  "dev_login": false,
  "oidc": false,
  "redirect_url": "https://suchlage.example.org/api/auth/oidc/callback",
  "setup_required": true
};

export const authOptionsOidc: AuthOptions = {
  "dev_login": false,
  "oidc": true,
  "redirect_url": "https://suchlage.example.org/api/auth/oidc/callback",
  "setup_required": false
};

export const health: Health = {
  "commit": "a5a73ed",
  "status": "ok",
  "version": "0.2.3"
};
