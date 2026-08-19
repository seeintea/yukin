UPDATE conversations
SET title = (
    SELECT trim(messages.content)
    FROM messages
    WHERE messages.conversation_id = conversations.id
      AND messages.role = 'user'
      AND length(trim(messages.content)) > 0
    ORDER BY messages.sequence
    LIMIT 1
)
WHERE title = '新对话'
  AND EXISTS (
      SELECT 1
      FROM messages
      WHERE messages.conversation_id = conversations.id
        AND messages.role = 'user'
        AND length(trim(messages.content)) > 0
  );
