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
    spike_profile: {
      executor: 'ramping-arrival-rate',
      startRate: envNumber('BASE_RATE', 20),
      timeUnit: '1s',
      preAllocatedVUs: envNumber('PREALLOCATED_VUS', 80),
      maxVUs: envNumber('MAX_VUS', 300),
      stages: [
        { target: envNumber('BASE_RATE', 20), duration: envString('BASE_DURATION', '10s') },
        { target: envNumber('SPIKE_RATE', 200), duration: envString('SPIKE_DURATION', '10s') },
        { target: envNumber('RECOVERY_RATE', envNumber('BASE_RATE', 20)), duration: envString('RECOVERY_DURATION', '10s') },
      ],
    },
  },
  thresholds: {
    http_req_failed: [`rate<${envNumber('FAIL_RATE', 0.05)}`],
    http_req_duration: [`p(99)<${envNumber('P99_MS', 1000)}`],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://127.0.0.1:8080';
const STATE_PATH = __ENV.STATE_PATH || '/api/v1/state';

export default function () {
  const response = http.get(`${BASE_URL}${STATE_PATH}`);
  check(response, {
    'state status is 200': (res) => res.status === 200,
  });
}
