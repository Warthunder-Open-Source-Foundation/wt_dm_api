import http from "k6/http";
import { check, sleep } from "k6";

// Test configuration
export const options = {
  thresholds: {
    // Assert that 99% of requests finish within 3000ms.
    http_req_duration: ["p(99) < 500"],
  },
  // Ramp the number of virtual users up and down
  stages: [
    { duration: "30s", target: 10000 },
  ],
};

// Simulated user behavior
export default function () {
  let res = http.get("http://localhost:3000/files/aces.vromfs.bin/gamedata/weapons/rocketguns/fr_mica_em.blk");
  // Validate response status
  check(res, { "status was 200": (r) => r.status == 200 });
  sleep(1);
}