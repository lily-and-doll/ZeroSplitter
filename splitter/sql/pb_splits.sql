SELECT score, hits, splits.run_id, mode
FROM (SELECT max(score_total), run_id, mode
FROM (SELECT sum(score) as score_total, *
FROM (SELECT score, run_id, mode
FROM splits 
INNER JOIN runs 
INNER JOIN categories
ON splits.run_id = runs.id AND runs.category = categories.id
WHERE categories.id = ?1)
GROUP BY run_id
ORDER BY score_total DESC)) AS sub
INNER JOIN splits
ON splits.run_id = sub.run_id