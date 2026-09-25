"""Explicit gateway parameter contract, shared by cases and reporting."""
import inspect

from openai.resources.responses import Responses

SUPPORTED_FIELDS = {
    'model', 'input', 'instructions', 'stream', 'store', 'tools', 'tool_choice',
    'parallel_tool_calls', 'reasoning', 'text', 'service_tier', 'prompt_cache_key', 'include',
}
TRANSPORT_FIELDS = {'extra_headers', 'extra_query', 'extra_body', 'timeout'}
SDK_FIELDS = set(inspect.signature(Responses.create).parameters) - TRANSPORT_FIELDS - {'self'}
REJECTED_FIELDS = sorted(SDK_FIELDS - SUPPORTED_FIELDS)
