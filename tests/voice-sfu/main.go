// Exercise the real backend SFU without a database or production credentials.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"github.com/pion/turn/v5"
	"net"
	"os"
	"papo/internal/config"
	voice "papo/internal/webrtc"
	"sync"
	"time"
)

type command struct {
	Peer  string `json:"peer"`
	Event struct {
		Type      string `json:"type"`
		SDP       string `json:"sdp"`
		Candidate string `json:"candidate"`
		MID       string `json:"sdp_mid"`
		Index     int    `json:"sdp_mline_index"`
		Muted     bool   `json:"muted"`
		On        bool   `json:"on"`
		Publisher string `json:"publisher_id"`
		Kind      string `json:"kind"`
	} `json:"event"`
}

func main() {
	var lock sync.Mutex
	send := func(peer string, event any) {
		lock.Lock()
		defer lock.Unlock()
		json.NewEncoder(os.Stdout).Encode(map[string]any{"peer": peer, "event": event})
	}
	var host string
	interfaces, _ := net.InterfaceAddrs()
	for _, a := range interfaces {
		ip, _, _ := net.ParseCIDR(a.String())
		if ip != nil && !ip.IsLoopback() && ip.To4() != nil {
			host = ip.String()
			break
		}
	}
	if host == "" {
		panic("a non-loopback interface is required by backend ICE validation")
	}
	udp, err := net.ListenPacket("udp4", host+":0")
	if err != nil {
		panic(err)
	}
	relay, err := turn.NewServer(turn.ServerConfig{Realm: "papo-test", AuthHandler: func(a *turn.RequestAttributes) (string, []byte, bool) {
		return a.Username, turn.GenerateAuthKey(a.Username, a.Realm, "probe+/password:="), a.Username == "1900000000:probe-user"
	}, PacketConnConfigs: []turn.PacketConnConfig{{PacketConn: udp, RelayAddressGenerator: &turn.RelayAddressGeneratorStatic{RelayAddress: net.ParseIP(host), Address: host}}}})
	if err != nil {
		panic(err)
	}
	defer relay.Close()
	cfg := &config.Config{VoiceVideoCodec: "vp8", VoiceVideoSlots: 6, VoiceAudioSlots: 8, VoiceMaxRoomPeers: 3, VoiceMaxRoomsPerUser: 1, VoiceRoomCleanupGrace: time.Second, VoiceSignalRateLimit: 100, VoiceSignalRateBurst: 200, VoiceSubscribeRateLimit: 100, VoiceSubscribeRateBurst: 200}
	m := voice.NewManager(cfg, voice.Signaler{SendToClient: send, SendToUser: send, VoiceAudience: func(string) map[string]bool { return map[string]bool{"a": true, "b": true, "c": true} }, BroadcastToUsers: func(_ map[string]bool, e any) { send("a", e); send("b", e); send("c", e) }})
	defer m.Shutdown()
	send("", map[string]any{"type": "ready", "ice_servers": []any{map[string]any{"urls": []string{fmt.Sprintf("turn:%s", udp.LocalAddr())}, "username": "1900000000:probe-user", "credential": "probe+/password:="}}})
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 4096), 256*1024)
	for scanner.Scan() {
		var c command
		if json.Unmarshal(scanner.Bytes(), &c) != nil {
			continue
		}
		var err error
		switch c.Event.Type {
		case "voice_join":
			err = m.Join("probe", c.Peer, c.Peer)
		case "voice_leave":
			err = m.Leave("probe", c.Peer, c.Peer)
		case "voice_offer":
			err = m.ClientOffer("probe", c.Peer, c.Peer, c.Event.SDP)
		case "voice_answer":
			err = m.ClientAnswer("probe", c.Peer, c.Peer, c.Event.SDP)
		case "voice_ice_candidate":
			err = m.AddICECandidate("probe", c.Peer, c.Peer, c.Event.Candidate, c.Event.MID, c.Event.Index)
		case "voice_mute":
			err = m.SetMuted("probe", c.Peer, c.Peer, c.Event.Muted)
		case "voice_camera":
			err = m.SetCameraOn("probe", c.Peer, c.Peer, c.Event.On)
		case "screen_share_start":
			err = m.StartScreenShare("probe", c.Peer, c.Peer)
		case "screen_share_stop":
			err = m.StopScreenShare("probe", c.Peer, c.Peer)
		case "track_subscribe":
			err = m.Subscribe("probe", c.Peer, c.Peer, c.Event.Publisher, c.Event.Kind)
		case "track_unsubscribe":
			err = m.Unsubscribe("probe", c.Peer, c.Peer, c.Event.Publisher, c.Event.Kind)
		case "disconnect":
			m.ClientOffline(c.Peer, c.Peer)
		}
		if err != nil {
			send(c.Peer, map[string]any{"type": "error", "code": voice.ErrorCode(err)})
		}
	}
}
