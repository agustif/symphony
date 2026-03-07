import http from 'k6/http';
import { check } from 'k6';

// Spike test - sudden burst of traffic
export const options = {
  stages: [
    { duration: '30s', target: 10 },   // Low baseline
    { duration: '10s', target: 1000 }, // Sudden spike
    { duration: '30s', target: 10 },   // Back to baseline
  ],
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

export default function () {
  const res = http.get(`${BASE_URL}/api/v1/state`);
  check(res, {
    'status is 200': (r) => r.status === 200,
  });
}
