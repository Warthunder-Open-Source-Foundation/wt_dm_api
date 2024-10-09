import http from "k6/http";
import {check, sleep} from "k6";

// Test configuration
export const options = {
    thresholds: {
        http_req_duration: ["p(95) < 250"],
    },
    vus: 50,
    duration: '20s',
};

// Simulated user behavior
export default function () {
    let res = http.get("http://localhost:3000/files/aces.vromfs.bin/gamedata/weapons/rocketguns");
    // Validate response status
    check(res, {"status was 200": (r) => r.status == 200});
    sleep(1);
}