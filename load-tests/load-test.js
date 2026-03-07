import http from 'k6/http';
import { check } from 'k6';

function envNumber(name, fallback) {
  const raw = __ENV[name];
  if (raw === undefined || raw === '') {
    return fallback;
  }

  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) {
    throw new Error(`Expected numeric env ${name}, got ${raw}`);
  }

  return parsed;
}

function envString(name, fallback) {
  const raw = __ENV[name];
  return raw === undefined || raw === '' ? fallback : raw;
}

export const options = {
  scenarios: {
    steady_state: {
      executor: 'constant-arrival-rate',
      rate: envNumber('RATE', 80),
      timeUnit: '1s',
      duration: envString('DURATION', '30s'),
      preAllocatedVUs: envNumber('PREALLOCATED_VUS', 40),
      maxVUs: envNumber('MAX_VUS', 200),
    },
  },
  thresholds: {
    http_req_duration: [`p(95)<${envNumber('P95_MS', 500)}`],
    http_req_failed: [`rate<${envNumber('FAIL_RATE', 0.01)}`],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:8080';
const STATE_PATH = __ENV.STATE_PATH || '/api/v1/state';

export default function () {
  const stateResponse = http.get(`${BASE_URL}${STATE_PATH}`);
  check(stateResponse, {
    'state status is 200': (response) => response.status === 200,
  });
}
