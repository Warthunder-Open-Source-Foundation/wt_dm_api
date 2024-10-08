import http from "k6/http";
import {check, sleep} from "k6";

// Test configuration
export const options = {
    thresholds: {
        http_req_duration: ["p(95) < 100"],
    },
    vus: 100,
    duration: '20s',
};

// Simulated user behavior
export default function () {
    let res = http.get("http://localhost:3000/files/aces.vromfs.bin/gamedata/weapons/rocketguns/fr_mica_em.blk");
    // Validate response status
    check(res, {"status was 200": (r) => r.status == 200});
    sleep('10ms');
}