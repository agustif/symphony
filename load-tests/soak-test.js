import http from 'k6/http';
import { check, sleep } from 'k6';

// Soak test - sustained load over time
export const options = {
  stages: [
    { duration: '10m', target: 100 },  // Ramp up
    { duration: '1h', target: 100 },   // Stay at 100 for 1 hour
    { duration: '10m', target: 0 },    // Ramp down
  ],
  thresholds: {
    http_req_failed: ['rate<0.01'],     // Very low error rate
    http_req_duration: ['p(95)<500'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export default function () {
  const res = http.get(`${BASE_URL}/api/v1/state`);
  check(res, {
    'status is 200': (r) => r.status === 200,
  });
  sleep(1);
}
