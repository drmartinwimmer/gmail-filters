{
  "version": "v1alpha3",
  "author": {
    "name": "Example User",
    "email": "user@example.com"
  },
  "labels": [
    { "name": "noise" },
    { "name": "lists/github" },
    { "name": "receipts" }
  ],
  "rules": [
    {
      "filter": {
        "or": [
          { "from": "promotions@marketing.example.com" },
          { "list": "digest.marketing.example.com" }
        ]
      },
      "actions": {
        "archive": true,
        "markRead": true,
        "labels": ["noise"]
      }
    },
    {
      "filter": {
        "list": "notifications.github.com"
      },
      "actions": {
        "archive": true,
        "labels": ["lists/github"]
      }
    },
    {
      "filter": {
        "from": "billing@service.example.com"
      },
      "actions": {
        "archive": true,
        "labels": ["receipts"]
      }
    },
    {
      "filter": {
        "and": [
          { "to": "user@example.com" },
          { "from": "teammate@work.example.com" }
        ]
      },
      "actions": {
        "archive": false,
        "markImportant": true
      }
    }
  ],
  "tests": [
    {
      "name": "tier1_bulk_marketing_routed_to_noise",
      "messages": [
        {
          "from": "promotions@marketing.example.com",
          "to": ["user@example.com"]
        }
      ],
      "actions": {
        "archive": true,
        "markRead": true,
        "labels": ["noise"]
      }
    },
    {
      "name": "tier2_github_notifications_labeled_and_archived",
      "messages": [
        {
          "lists": ["notifications.github.com"]
        }
      ],
      "actions": {
        "archive": true,
        "labels": ["lists/github"]
      }
    },
    {
      "name": "tier3_direct_teammate_email_stays_in_inbox",
      "messages": [
        {
          "from": "teammate@work.example.com",
          "to": ["user@example.com"]
        }
      ],
      "actions": {
        "archive": false,
        "markImportant": true
      }
    }
  ]
}
