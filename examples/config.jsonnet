// Synthetic example gmailctl configuration (v1alpha3) demonstrating the three-tier triage model.
// Personal configuration must live in a separate private repository.
local lib = import 'gmailctl.libsonnet';

{
  version: 'v1alpha3',
  author: {
    name: 'Example User',
    email: 'user@example.com',
  },
  labels: [
    { name: 'noise' },
    { name: 'lists/github' },
    { name: 'receipts' },
  ],
  rules: [
    // Tier 1: Deterministic noise routing (unopened / bulk senders without unsubscribe)
    {
      filter: {
        or: [
          { from: 'promotions@marketing.example.com' },
          { list: 'digest.marketing.example.com' },
        ],
      },
      actions: {
        archive: true,
        markRead: true,
        labels: ['noise'],
      },
    },

    // Tier 2: Labeled & skip-inbox (proposed by sandboxed local LLM classifier, codified as deterministic rule)
    {
      filter: {
        list: 'notifications.github.com',
      },
      actions: {
        archive: true,
        labels: ['lists/github'],
      },
    },
    {
      filter: {
        from: 'billing@service.example.com',
      },
      actions: {
        archive: true,
        labels: ['receipts'],
      },
    },

    // Tier 3: Actionable / direct correspondence stays in the main inbox (archive: false)
    {
      filter: {
        and: [
          { to: 'user@example.com' },
          { from: 'teammate@work.example.com' },
        ],
      },
      actions: {
        archive: false,
        markImportant: true,
      },
    },
  ],
  tests: [
    {
      name: 'tier1_bulk_marketing_routed_to_noise',
      messages: [
        {
          from: 'promotions@marketing.example.com',
          to: ['user@example.com'],
        },
      ],
      actions: {
        archive: true,
        markRead: true,
        labels: ['noise'],
      },
    },
    {
      name: 'tier2_github_notifications_labeled_and_archived',
      messages: [
        {
          lists: ['notifications.github.com'],
        },
      ],
      actions: {
        archive: true,
        labels: ['lists/github'],
      },
    },
    {
      name: 'tier3_direct_teammate_email_stays_in_inbox',
      messages: [
        {
          from: 'teammate@work.example.com',
          to: ['user@example.com'],
        },
      ],
      actions: {
        archive: false,
        markImportant: true,
      },
    },
  ],
}
