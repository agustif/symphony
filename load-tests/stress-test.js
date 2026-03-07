import http from 'k6/http';
import { check, sleep } from 'k6';

// Stress test - push to breaking point
export const options = {
  stages: [
    { duration: '2m', target: 500 },   // Ramp up to 500 users
    { duration: '5m', target: 1000 },  // Ramp up to 1000 users
    { duration: '10m', target: 1000 }, // Hold at 1000 users
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(99)<1000'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export default function () {
  const res = http.get(`${BASE_URL}/api/v1/state`);
  check(res, {
    'status is 200': (r) => r.status === 200,
  });
  sleep(0.1);
}
