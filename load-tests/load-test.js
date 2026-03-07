import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  stages: [
    { duration: '2m', target: 100 }, // Ramp up to 100 users
    { duration: '5m', target: 100 }, // Stay at 100 users
    { duration: '2m', target: 200 }, // Ramp up to 200 users
    { duration: '5m', target: 200 }, // Stay at 200 users
    { duration: '2m', target: 0 },   // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% of requests under 500ms
    http_req_failed: ['rate<0.1'],     // Error rate under 0.1%
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export default function () {
  // Test state endpoint
  const stateRes = http.get(`${BASE_URL}/api/v1/state`);
  check(stateRes, {
    'state status is 200': (r) => r.status === 200,
    'state response time < 500ms': (r) => r.timings.duration < 500,
  });

  sleep(1);

  // Test dashboard
  const dashboardRes = http.get(`${BASE_URL}/`);
  check(dashboardRes, {
    'dashboard status is 200': (r) => r.status === 200,
  });

  sleep(1);
}
