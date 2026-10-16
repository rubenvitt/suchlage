import { setupServer } from 'msw/node';
import { meOperator } from '../contract/fixtures.generated';
import { handlersFor } from './handlers';

export const server = setupServer(...handlersFor(meOperator));
