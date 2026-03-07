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
    stress_ramp: {
      executor: 'ramping-arrival-rate',
      startRate: envNumber('START_RATE', 50),
      timeUnit: '1s',
      preAllocatedVUs: envNumber('PREALLOCATED_VUS', 100),
      maxVUs: envNumber('MAX_VUS', 500),
      stages: [
        { target: envNumber('STAGE1_RATE', 100), duration: envString('STAGE1_DURATION', '20s') },
        { target: envNumber('STAGE2_RATE', 200), duration: envString('STAGE2_DURATION', '20s') },
        { target: envNumber('STAGE3_RATE', 300), duration: envString('STAGE3_DURATION', '20s') },
        { target: 0, duration: envString('RAMP_DOWN_DURATION', '10s') },
      ],
    },
  },
  thresholds: {
    http_req_duration: [`p(99)<${envNumber('P99_MS', 1000)}`],
    http_req_failed: [`rate<${envNumber('FAIL_RATE', 0.05)}`],
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
