SELECT max(score)
FROM (SELECT split_num, score, mode
FROM splits 
INNER JOIN runs 
INNER JOIN categories
ON splits.run_id = runs.id AND runs.category = categories.id
WHERE categories.id = ?1) GROUP BY split_num