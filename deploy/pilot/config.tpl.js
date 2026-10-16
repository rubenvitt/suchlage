export default {
  appId: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/username }}",
  appKey: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/credential }}",
  license: "{{ op://DRK/zy32h44zpdllk7vttcyi33ldce/license }}",
  workspaceId: "768d4a2e-1f34-477c-9fcc-872285e895f6",
  mqttHost: "tcp://__BIND_IP__:1883",
  mqttUser: "rc-plus-01",
  mqttPass: "{{ op://DRK/suchlage-mqtt/rc-plus-01 }}",
  rtmpUrl: "rtmp://__BIND_IP__:1935/live/m350?user=pilot&pass={{ op://DRK/suchlage-mqtt/rtmp-pilot }}",
};
